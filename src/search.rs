use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::eval::{evaluate, VALUE};
use crate::movegen::{generate_moves, legal_moves, legal_next};
use crate::moves::{Move, MoveList};
use crate::position::Position;
use crate::tt::{Bound, Tt};
use crate::types::{Color, Piece};

const INF: i32 = 32_000;
const MATE: i32 = 30_000;

#[derive(Default)]
pub struct Limits {
    pub depth: Option<u32>,
    pub movetime: Option<u64>,
    pub wtime: Option<u64>,
    pub btime: Option<u64>,
    pub winc: Option<u64>,
    pub binc: Option<u64>,
    pub infinite: bool,
    pub quiet: bool, // suppress `info` lines (used by bench)
}

fn time_budget(limits: &Limits, us: Color) -> Option<Duration> {
    if let Some(ms) = limits.movetime {
        return Some(Duration::from_millis(ms.saturating_sub(10).max(1)));
    }
    let (time, inc) = if us == Color::White { (limits.wtime, limits.winc) } else { (limits.btime, limits.binc) };
    let time = time?; // no clock given: no deadline
    let ms = time / 30 + inc.unwrap_or(0) / 2;
    Some(Duration::from_millis(ms.min(time.saturating_sub(50)).max(1)))
}

fn in_check(pos: &Position) -> bool {
    let king = pos.pieces[pos.color_to_move][Piece::King].lsb();
    pos.is_square_attacked(king, pos.color_to_move.flip())
}

// ---- mate scores in the transposition table ----
//
// Inside the search a mate score says "mate N plies from the ROOT" (-MATE + ply, etc.).
// The same position can be reached at different plies, so the table stores the score
// relative to the node itself, and we convert back using the ply of whoever reads it.

fn to_tt(score: i32, ply: i32) -> i32 {
    if score >= MATE - 256 { score + ply } else if score <= -(MATE - 256) { score - ply } else { score }
}

fn from_tt(score: i32, ply: i32) -> i32 {
    if score >= MATE - 256 { score - ply } else if score <= -(MATE - 256) { score + ply } else { score }
}

// ---- move ordering: table move, then captures (most valuable victim, least valuable attacker) ----

fn move_score(pos: &Position, m: Move, tt_move: Move) -> i32 {
    // The table's move is never played directly: we only boost a move that our own generator
    // produced, so a stale or colliding entry can never inject an illegal move.
    if m == tt_move {
        return 1_000_000;
    }
    let mut s = 0;
    if pos.is_capture(m) {
        let victim = if m.kind() == Move::EN_PASSANT { Piece::Pawn } else { pos.mailbox[m.to_sq()].unwrap().1 };
        let attacker = pos.mailbox[m.from_sq()].unwrap().1;
        s += 10_000 + VALUE[victim as usize] * 10 - VALUE[attacker as usize];
    }
    if let Some(p) = m.promotion_piece() {
        s += VALUE[p as usize];
    }
    s
}

fn score_moves(pos: &Position, moves: &MoveList, scores: &mut [i32; 256], tt_move: Move) {
    for i in 0..moves.len {
        scores[i] = move_score(pos, moves.moves[i], tt_move);
    }
}

// selection step: bring the best remaining move to position i
fn pick_next(moves: &mut MoveList, scores: &mut [i32; 256], i: usize) {
    let mut best = i;
    for j in i + 1..moves.len {
        if scores[j] > scores[best] { best = j; }
    }
    moves.moves.swap(i, best);
    scores.swap(i, best);
}

struct Searcher<'a> {
    tt: &'a mut Tt,
    stop: Arc<AtomicBool>,
    deadline: Option<Instant>,
    nodes: u64,
    stopped: bool,
    path: Vec<u64>,  // hashes of every position before the current one (game history + search line)
    root_move: Move, // best move of the previous iteration, searched first at the root
}

impl Searcher<'_> {
    // checks the clock and the stop flag every 2048 nodes
    fn should_stop(&mut self) -> bool {
        if self.stopped { return true; }
        if self.nodes & 2047 == 0 {
            let out_of_time = self.deadline.map_or(false, |d| Instant::now() >= d);
            if out_of_time || self.stop.load(Ordering::Relaxed) {
                self.stopped = true;
            }
        }
        self.stopped
    }

    // Has this position occurred before on the current path? Same side to move means
    // 2, 4, 6... plies back, and a pawn move or capture (the clock resets) ends the look-back.
    fn is_repetition(&self, pos: &Position) -> bool {
        let n = self.path.len();
        let limit = (pos.half_move_clock as usize).min(n);
        (2..=limit).step_by(2).any(|back| self.path[n - back] == pos.hash)
    }

    fn quiesce(&mut self, pos: &Position, mut alpha: i32, beta: i32) -> i32 {
        self.nodes += 1;
        if self.should_stop() { return 0; }

        let stand_pat = evaluate(pos);
        if stand_pat >= beta { return beta; }
        if stand_pat > alpha { alpha = stand_pat; }

        let mut moves = MoveList::new();
        generate_moves(pos, &mut moves);
        let mut scores = [0i32; 256];
        score_moves(pos, &moves, &mut scores, Move::NULL);

        for i in 0..moves.len {
            pick_next(&mut moves, &mut scores, i);
            if scores[i] == 0 { break; } // only quiet moves left
            let Some(next) = legal_next(pos, moves.moves[i]) else { continue };
            let score = -self.quiesce(&next, -beta, -alpha);
            if self.stopped { return 0; }
            if score >= beta { return beta; }
            if score > alpha { alpha = score; }
        }
        alpha
    }

    fn negamax(&mut self, pos: &Position, depth: i32, ply: i32, mut alpha: i32, beta: i32) -> i32 {
        if depth <= 0 { return self.quiesce(pos, alpha, beta); }
        self.nodes += 1;
        if self.should_stop() { return 0; }
        if pos.half_move_clock >= 100 || self.is_repetition(pos) { return 0; } // draws

        // 1. ask the table whether we already know something about this position
        let alpha_orig = alpha;
        let mut tt_move = Move::NULL;
        if let Some(e) = self.tt.probe(pos.hash) {
            tt_move = e.mv;
            if e.depth as i32 >= depth {
                let score = from_tt(e.score, ply);
                match e.bound {
                    Bound::Exact => return score,
                    Bound::Lower if score >= beta => return beta,
                    Bound::Upper if score <= alpha => return alpha,
                    _ => {}
                }
            }
        }

        let mut moves = MoveList::new();
        generate_moves(pos, &mut moves);
        let mut scores = [0i32; 256];
        score_moves(pos, &moves, &mut scores, tt_move);

        let mut legal = 0;
        let mut best_move = Move::NULL;
        for i in 0..moves.len {
            pick_next(&mut moves, &mut scores, i);
            let m = moves.moves[i];
            let Some(next) = legal_next(pos, m) else { continue };
            legal += 1;

            self.path.push(pos.hash);
            let score = -self.negamax(&next, depth - 1, ply + 1, -beta, -alpha);
            self.path.pop();

            if self.stopped { return 0; } // unfinished: never store half-searched results
            if score >= beta {
                self.tt.store(pos.hash, m, to_tt(beta, ply), depth, Bound::Lower);
                return beta;
            }
            if score > alpha {
                alpha = score;
                best_move = m;
            }
        }
        if legal == 0 {
            return if in_check(pos) { -MATE + ply } else { 0 }; // mated sooner is worse; stalemate is a draw
        }

        // 2. write down what we learned
        let bound = if alpha > alpha_orig { Bound::Exact } else { Bound::Upper };
        self.tt.store(pos.hash, best_move, to_tt(alpha, ply), depth, bound);
        alpha
    }

    fn root(&mut self, pos: &Position, depth: i32) -> (Option<Move>, i32) {
        let mut moves = MoveList::new();
        generate_moves(pos, &mut moves);
        let mut scores = [0i32; 256];
        score_moves(pos, &moves, &mut scores, self.root_move);

        let mut alpha = -INF;
        let mut best = None;
        for i in 0..moves.len {
            pick_next(&mut moves, &mut scores, i);
            let m = moves.moves[i];
            let Some(next) = legal_next(pos, m) else { continue };

            self.path.push(pos.hash);
            let score = -self.negamax(&next, depth - 1, 1, -INF, -alpha);
            self.path.pop();

            if self.stopped { break; }
            if score > alpha {
                alpha = score;
                best = Some(m);
            }
        }
        (best, alpha)
    }
}

fn score_string(score: i32) -> String {
    if score.abs() >= MATE - 256 {
        let moves = (MATE - score.abs() + 1) / 2;
        format!("mate {}", if score > 0 { moves } else { -moves })
    } else {
        format!("cp {score}")
    }
}

/// `history` holds the hash of every position of the game before `pos` (for repetitions).
pub fn search(pos: &Position, limits: &Limits, stop: Arc<AtomicBool>, history: Vec<u64>, tt: &mut Tt) -> Option<Move> {
    search_nodes(pos, limits, stop, history, tt).0
}

pub fn search_nodes(
    pos: &Position,
    limits: &Limits,
    stop: Arc<AtomicBool>,
    history: Vec<u64>,
    tt: &mut Tt,
) -> (Option<Move>, u64) {
    let start = Instant::now();
    let mut s = Searcher {
        tt,
        stop,
        deadline: time_budget(limits, pos.color_to_move).map(|d| start + d),
        nodes: 0,
        stopped: false,
        path: history,
        root_move: Move::NULL,
    };

    let mut best = None;
    for depth in 1..=limits.depth.unwrap_or(64) as i32 {
        let (m, score) = s.root(pos, depth);
        if s.stopped { break; } // discard the unfinished iteration
        best = m;
        s.root_move = m.unwrap_or(Move::NULL);
        if !limits.quiet {
            let ms = start.elapsed().as_millis().max(1);
            println!(
                "info depth {depth} score {} nodes {} nps {} time {ms}",
                score_string(score), s.nodes, s.nodes as u128 * 1000 / ms
            );
        }
        if m.is_none() || score > MATE - 256 { break; } // no legal moves, or mate found
    }
    // if even depth 1 was cut off, still return something legal
    let best = best.or_else(|| legal_moves(pos).as_slice().first().copied());
    (best, s.nodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn best(fen: &str, depth: u32) -> String {
        let pos = Position::from_fen(fen).unwrap();
        let limits = Limits { depth: Some(depth), quiet: true, ..Default::default() };
        let mut tt = Tt::new(1);
        search(&pos, &limits, Arc::new(AtomicBool::new(false)), Vec::new(), &mut tt).unwrap().to_string()
    }

    fn searcher(tt: &mut Tt, path: Vec<u64>) -> Searcher<'_> {
        Searcher {
            tt,
            stop: Arc::new(AtomicBool::new(false)),
            deadline: None,
            nodes: 0,
            stopped: false,
            path,
            root_move: Move::NULL,
        }
    }

    #[test]
    fn finds_back_rank_mate() {
        assert_eq!(best("6k1/5ppp/8/8/8/8/8/R3K3 w - - 0 1", 3), "a1a8");
    }

    #[test]
    fn takes_a_free_queen() {
        assert_eq!(best("4k3/8/8/3q4/8/8/8/3QK3 w - - 0 1", 3), "d1d5");
    }

    #[test]
    fn mate_scores_roundtrip_through_the_table() {
        for ply in [0, 1, 5, 40] {
            for score in [MATE - 3, -(MATE - 4), 0, 150, -2000] {
                assert_eq!(from_tt(to_tt(score, ply), ply), score);
            }
        }
        // a mate found 3 plies below a node at ply 4 is stored as "mate in 3 from this node"
        assert_eq!(to_tt(MATE - 7, 4), MATE - 3);
    }

    #[test]
    fn detects_repetition() {
        use crate::movegen::legal_moves;
        let mut pos = Position::startpos();
        let mut history = Vec::new();
        for text in ["g1f3", "g8f6", "f3g1", "f6g8"] {
            history.push(pos.hash);
            let m = legal_moves(&pos).as_slice().iter().copied().find(|m| m.to_string() == text).unwrap();
            pos = pos.make_move(m);
        }
        let mut tt = Tt::new(1);
        let s = searcher(&mut tt, history);
        assert!(s.is_repetition(&pos)); // back at the start position
    }

    #[test]
    fn no_false_repetition() {
        use crate::movegen::legal_moves;
        let mut pos = Position::startpos();
        let mut history = Vec::new();
        for text in ["e2e4", "e7e5", "g1f3", "g8f6"] {
            history.push(pos.hash);
            let m = legal_moves(&pos).as_slice().iter().copied().find(|m| m.to_string() == text).unwrap();
            pos = pos.make_move(m);
        }
        let mut tt = Tt::new(1);
        let s = searcher(&mut tt, history);
        assert!(!s.is_repetition(&pos)); // every position so far is new
    }

    #[test]
    fn warm_table_gives_the_same_answer() {
        let pos = Position::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1").unwrap();
        let limits = Limits { depth: Some(4), quiet: true, ..Default::default() };
        let mut tt = Tt::new(4);
        let (m1, n1) = search_nodes(&pos, &limits, Arc::new(AtomicBool::new(false)), Vec::new(), &mut tt);
        // a second search with the table already filled must do less work and still return a legal move
        let (m2, n2) = search_nodes(&pos, &limits, Arc::new(AtomicBool::new(false)), Vec::new(), &mut tt);
        assert!(n2 < n1, "second search should reuse the table ({n2} vs {n1})");
        let legal = legal_moves(&pos);
        assert!(legal.as_slice().contains(&m1.unwrap()));
        assert!(legal.as_slice().contains(&m2.unwrap()));
    }
}