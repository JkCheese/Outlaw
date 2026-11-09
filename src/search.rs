use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::eval::{evaluate, VALUE};
use crate::movegen::{generate_captures, generate_moves, legal_moves, legal_next};
use crate::moves::{Move, MoveList};
use crate::position::Position;
use crate::tt::{Bound, Tt};
use crate::types::{Color, Piece};

const INF: i32 = 32_000;
const MATE: i32 = 30_000;
const MAX_PLY: usize = 128; // hard cap on search depth (extensions included)

#[derive(Default)]
pub struct Limits {
    pub depth: Option<u32>,
    pub movetime: Option<u64>,
    pub wtime: Option<u64>,
    pub btime: Option<u64>,
    pub winc: Option<u64>,
    pub binc: Option<u64>,
    pub infinite: bool,
    pub nodes: Option<u64>, // `go nodes N`: stop after about N nodes
    pub quiet: bool,        // suppress `info` lines (used by bench)
    pub chess960: bool,     // how castling is printed in `info ... pv`
}

/// Returns (soft, hard) time limits for this move.
///  * hard: the search is cut off here no matter what (a safety net).
///  * soft: the budget we aim for; after a finished depth we don't start another one
///    if it probably couldn't finish inside it. `movetime` has no soft limit: use it all.
fn time_limits(limits: &Limits, us: Color) -> (Option<Duration>, Option<Duration>) {
    if let Some(ms) = limits.movetime {
        return (None, Some(Duration::from_millis(ms.saturating_sub(10).max(1))));
    }
    let (time, inc) = if us == Color::White { (limits.wtime, limits.winc) } else { (limits.btime, limits.binc) };
    let Some(time) = time else { return (None, None) }; // no clock given: no deadline
    let inc = inc.unwrap_or(0);
    let usable = time.saturating_sub(50).max(1); // keep a margin for GUI and process overhead
    let soft = (time / 30 + inc / 2).min(usable).max(1);
    let hard = (soft * 3).min(time / 4).max(soft).min(usable);
    (Some(Duration::from_millis(soft)), Some(Duration::from_millis(hard)))
}

// Does `c` have anything besides pawns and the king? Without such pieces a "pass" can be the
// worst move (zugzwang), so null-move pruning must not be used.
fn has_non_pawn_material(pos: &Position, c: Color) -> bool {
    let p = &pos.pieces[c];
    !(p[Piece::Knight] | p[Piece::Bishop] | p[Piece::Rook] | p[Piece::Queen]).is_empty()
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
    max_nodes: Option<u64>,
    nodes: u64,
    stopped: bool,
    path: Vec<u64>,  // hashes of every position before the current one (game history + search line)
    root_move: Move, // best move of the previous iteration, searched first at the root
    // Principal variation, "triangular" layout: row `ply` holds the best line from that ply
    // on (pv[ply * MAX_PLY ..][..pv_len[ply]]). A parent builds its row from its child's.
    pv: Vec<Move>,
    pv_len: [usize; MAX_PLY + 1],
}

impl<'a> Searcher<'a> {
    fn new(tt: &'a mut Tt, stop: Arc<AtomicBool>, deadline: Option<Instant>, max_nodes: Option<u64>, path: Vec<u64>) -> Self {
        Searcher {
            tt,
            stop,
            deadline,
            max_nodes,
            nodes: 0,
            stopped: false,
            path,
            root_move: Move::NULL,
            pv: vec![Move::NULL; (MAX_PLY + 1) * MAX_PLY],
            pv_len: [0; MAX_PLY + 1],
        }
    }

    // `m` just raised alpha at `ply`: the line from here is `m` followed by the child's line
    fn update_pv(&mut self, ply: usize, m: Move) {
        let child_len = self.pv_len[ply + 1];
        self.pv[ply * MAX_PLY] = m;
        for i in 0..child_len {
            self.pv[ply * MAX_PLY + 1 + i] = self.pv[(ply + 1) * MAX_PLY + i];
        }
        self.pv_len[ply] = child_len + 1;
    }

    // checks the clock and the stop flag every 2048 nodes
    fn should_stop(&mut self) -> bool {
        if self.stopped { return true; }
        if self.nodes & 2047 == 0 {
            let out_of_time = self.deadline.map_or(false, |d| Instant::now() >= d);
            let out_of_nodes = self.max_nodes.map_or(false, |n| self.nodes >= n);
            if out_of_time || out_of_nodes || self.stop.load(Ordering::Relaxed) {
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
        generate_captures(pos, &mut moves); // captures and promotions only: no quiet moves to throw away
        let mut scores = [0i32; 256];
        score_moves(pos, &moves, &mut scores, Move::NULL);

        for i in 0..moves.len {
            pick_next(&mut moves, &mut scores, i);
            let Some(next) = legal_next(pos, moves.moves[i]) else { continue };
            let score = -self.quiesce(&next, -beta, -alpha);
            if self.stopped { return 0; }
            if score >= beta { return beta; }
            if score > alpha { alpha = score; }
        }
        alpha
    }

    fn negamax(&mut self, pos: &Position, mut depth: i32, ply: i32, mut alpha: i32, beta: i32, can_null: bool) -> i32 {
        let p = ply as usize;
        if p >= MAX_PLY { return evaluate(pos); }
        self.pv_len[p] = 0; // no line known from this node yet (must happen before any early return)

        // Check extension: when in check, search one ply deeper. Without it a check at the
        // horizon is evaluated as if nothing happened, hiding mates and forced replies.
        let checked = in_check(pos);
        if checked { depth += 1; }
        if depth <= 0 { return self.quiesce(pos, alpha, beta); }

        self.nodes += 1;
        if self.should_stop() { return 0; }
        // draws: fifty-move rule, dead material, repetition
        if pos.half_move_clock >= 100 || pos.insufficient_material() || self.is_repetition(pos) { return 0; }

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

        // 2. null-move pruning: if even after PASSING (giving the opponent a free move) the position
        // still looks good enough to cause a cutoff, a real move would too, so skip the search.
        // Only in non-PV nodes (zero-width window), never in check, never twice in a row, and never
        // without pieces (zugzwang: passing can be the best move there).
        if can_null && !checked && depth >= 3 && beta - alpha == 1
            && has_non_pawn_material(pos, pos.color_to_move)
            && evaluate(pos) >= beta
        {
            let reduction = 3 + depth / 3;
            let next = pos.make_null_move();
            self.path.push(pos.hash);
            let score = -self.negamax(&next, depth - 1 - reduction, ply + 1, -beta, -beta + 1, false);
            self.path.pop();
            if self.stopped { return 0; }
            // a "mate" found after passing can't be trusted: just search normally
            if score >= beta && score < MATE - 256 { return beta; }
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

            // Principal variation search: the first move is searched with the full window. Every
            // later move is expected to be worse, so prove that with a zero-width window (much
            // cheaper), and only search it properly if it unexpectedly beats alpha.
            self.path.push(pos.hash);
            let score = if legal == 1 {
                -self.negamax(&next, depth - 1, ply + 1, -beta, -alpha, true)
            } else {
                let s = -self.negamax(&next, depth - 1, ply + 1, -alpha - 1, -alpha, true);
                if s > alpha && s < beta && !self.stopped {
                    -self.negamax(&next, depth - 1, ply + 1, -beta, -alpha, true)
                } else {
                    s
                }
            };
            self.path.pop();

            if self.stopped { return 0; } // unfinished: never store half-searched results
            if score >= beta {
                self.tt.store(pos.hash, m, to_tt(beta, ply), depth, Bound::Lower);
                return beta;
            }
            if score > alpha {
                alpha = score;
                best_move = m;
                self.update_pv(p, m);
            }
        }
        if legal == 0 {
            return if checked { -MATE + ply } else { 0 }; // mated sooner is worse; stalemate is a draw
        }

        // 2. write down what we learned
        let bound = if alpha > alpha_orig { Bound::Exact } else { Bound::Upper };
        self.tt.store(pos.hash, best_move, to_tt(alpha, ply), depth, bound);
        alpha
    }

    fn root(&mut self, pos: &Position, depth: i32) -> (Option<Move>, i32) {
        self.pv_len[0] = 0;
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

            // same principal-variation search as in negamax: first move full window, rest zero-width
            self.path.push(pos.hash);
            let score = if best.is_none() {
                -self.negamax(&next, depth - 1, 1, -INF, -alpha, true)
            } else {
                let s = -self.negamax(&next, depth - 1, 1, -alpha - 1, -alpha, true);
                if s > alpha && !self.stopped {
                    -self.negamax(&next, depth - 1, 1, -INF, -alpha, true)
                } else {
                    s
                }
            };
            self.path.pop();

            if self.stopped { break; }
            if score > alpha {
                alpha = score;
                best = Some(m);
                self.update_pv(0, m);
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
    let (soft, hard) = time_limits(limits, pos.color_to_move);
    let mut s = Searcher::new(tt, stop, hard.map(|d| start + d), limits.nodes, history);

    let mut best = None;
    for depth in 1..=limits.depth.unwrap_or(64) as i32 {
        let (m, score) = s.root(pos, depth);

        // Keep the answer even from an UNFINISHED iteration. The previous best move is searched
        // first, so by the time anything else replaces it, that move beat it at this deeper
        // depth. (`root` only reports moves whose searches fully completed.)
        if let Some(m) = m {
            best = Some(m);
            s.root_move = m;
        }
        if s.stopped { break; }

        let elapsed = start.elapsed();
        if !limits.quiet {
            let ms = elapsed.as_millis().max(1);
            let pv: Vec<String> = s.pv[..s.pv_len[0]].iter().map(|m| m.to_uci(limits.chess960)).collect();
            println!(
                "info depth {depth} score {} nodes {} nps {} hashfull {} time {ms} pv {}",
                score_string(score), s.nodes, s.nodes as u128 * 1000 / ms, s.tt.hashfull(), pv.join(" ")
            );
        }
        if m.is_none() || score > MATE - 256 { break; } // no legal moves, or mate found
        // Stop once the soft budget is used up. (An unfinished next depth is still useful, see above,
        // and the hard limit cuts it off, so there is no need to guess whether it will finish.)
        if soft.map_or(false, |soft| elapsed >= soft) { break; }
    }
    // if even the first root move was cut off, still return something legal
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
        Searcher::new(tt, Arc::new(AtomicBool::new(false)), None, None, path)
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

    #[test]
    fn check_extension_sees_mate_at_the_horizon() {
        // Ra8# is only visible at depth 1 if the checked position is searched one ply further
        let pos = Position::from_fen("6k1/5ppp/8/8/8/8/8/R3K3 w - - 0 1").unwrap();
        let mut tt = Tt::new(1);
        let mut s = searcher(&mut tt, Vec::new());
        let (m, score) = s.root(&pos, 1);
        assert_eq!(m.unwrap().to_string(), "a1a8");
        assert!(score > MATE - 10, "score {score}");
    }

    #[test]
    fn pv_is_a_legal_line_starting_with_the_best_move() {
        for (fen, depth) in [
            ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 5),
            ("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", 4),
            ("6k1/5ppp/8/8/8/8/8/R3K3 w - - 0 1", 4),
        ] {
            let mut pos = Position::from_fen(fen).unwrap();
            let mut tt = Tt::new(4);
            let mut s = searcher(&mut tt, Vec::new());
            let (best, _) = s.root(&pos, depth);
            let line: Vec<Move> = s.pv[..s.pv_len[0]].to_vec();
            assert!(!line.is_empty());
            assert_eq!(line[0], best.unwrap());
            for m in line {
                assert!(legal_moves(&pos).as_slice().contains(&m), "illegal PV move {m} in {fen}");
                pos = pos.make_move(m);
            }
        }
    }

    #[test]
    fn stopping_mid_iteration_still_returns_a_legal_move() {
        let pos = Position::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1").unwrap();
        let legal = legal_moves(&pos);
        // node limits that stop the search at many different points, including inside iterations
        for nodes in [1, 2100, 4200, 9000, 20_000, 50_000, 123_456] {
            let limits = Limits { nodes: Some(nodes), quiet: true, ..Default::default() };
            let mut tt = Tt::new(1);
            let (m, used) = search_nodes(&pos, &limits, Arc::new(AtomicBool::new(false)), Vec::new(), &mut tt);
            assert!(legal.as_slice().contains(&m.unwrap()), "limit {nodes}");
            assert!(used <= nodes + 2048 * 2, "limit {nodes} used {used}"); // checked every 2048 nodes
        }
    }

    #[test]
    fn time_limits_behave() {
        use std::time::Duration;
        // movetime: all of it (minus a margin), no soft limit
        let l = Limits { movetime: Some(500), ..Default::default() };
        assert_eq!(time_limits(&l, Color::White), (None, Some(Duration::from_millis(490))));
        // no clock at all: no limits
        assert_eq!(time_limits(&Limits::default(), Color::White), (None, None));
        // a normal clock: soft < hard, hard well under the time left
        let l = Limits { wtime: Some(8000), winc: Some(80), btime: Some(1000), binc: Some(0), ..Default::default() };
        let (soft, hard) = time_limits(&l, Color::White);
        assert_eq!((soft.unwrap().as_millis(), hard.unwrap().as_millis()), (306, 918));
        // the side to move decides which clock is used
        assert_eq!(time_limits(&l, Color::Black).0.unwrap().as_millis(), 33);
        // nearly out of time: never ask for more than we have
        let l = Limits { wtime: Some(40), ..Default::default() };
        let (soft, hard) = time_limits(&l, Color::White);
        assert!(soft.unwrap() <= hard.unwrap() && hard.unwrap().as_millis() <= 40);
    }

    #[test]
    fn finds_classic_mates_with_pvs_and_null_move() {
        // (fen, expected best move, depth)
        for (fen, mv, depth) in [
            ("r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4", "h5f7", 3), // Scholar's mate
            ("rnbqkbnr/pppp1ppp/8/4p3/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq - 0 2", "d8h4", 3),      // Fool's mate
            ("6rk/6pp/8/6N1/8/8/8/6K1 w - - 0 1", "g5f7", 4),                                  // smothered mate
        ] {
            assert_eq!(best(fen, depth), mv, "{fen}");
        }
    }

    #[test]
    fn non_pawn_material_guard() {
        // null-move pruning is switched off for a side with only king and pawns (zugzwang risk)
        let pos = Position::from_fen("8/8/3k4/8/3K4/3P4/8/8 w - - 0 1").unwrap();
        assert!(!has_non_pawn_material(&pos, Color::White));
        assert!(!has_non_pawn_material(&pos, Color::Black));
        let pos2 = Position::from_fen("4k3/8/8/8/8/8/8/3QK3 w - - 0 1").unwrap();
        assert!(has_non_pawn_material(&pos2, Color::White));
        assert!(!has_non_pawn_material(&pos2, Color::Black));
    }
} // mod tests