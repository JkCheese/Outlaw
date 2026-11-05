use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::eval::{evaluate, VALUE};
use crate::movegen::{generate_moves, legal_moves, legal_next};
use crate::moves::{Move, MoveList};
use crate::position::Position;
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

// ---- move ordering: captures first (most valuable victim, least valuable attacker) ----

fn move_score(pos: &Position, m: Move) -> i32 {
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

fn score_moves(pos: &Position, moves: &MoveList, scores: &mut [i32; 256]) {
    for i in 0..moves.len {
        scores[i] = move_score(pos, moves.moves[i]);
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

struct Searcher {
    stop: Arc<AtomicBool>,
    deadline: Option<Instant>,
    nodes: u64,
    stopped: bool,
}

impl Searcher {
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

    fn quiesce(&mut self, pos: &Position, mut alpha: i32, beta: i32) -> i32 {
        self.nodes += 1;
        if self.should_stop() { return 0; }

        let stand_pat = evaluate(pos);
        if stand_pat >= beta { return beta; }
        if stand_pat > alpha { alpha = stand_pat; }

        let mut moves = MoveList::new();
        generate_moves(pos, &mut moves);
        let mut scores = [0i32; 256];
        score_moves(pos, &moves, &mut scores);

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
        if pos.half_move_clock >= 100 { return 0; } // fifty-move draw (repetition comes later)

        let mut moves = MoveList::new();
        generate_moves(pos, &mut moves);
        let mut scores = [0i32; 256];
        score_moves(pos, &moves, &mut scores);

        let mut legal = 0;
        for i in 0..moves.len {
            pick_next(&mut moves, &mut scores, i);
            let Some(next) = legal_next(pos, moves.moves[i]) else { continue };
            legal += 1;
            let score = -self.negamax(&next, depth - 1, ply + 1, -beta, -alpha);
            if self.stopped { return 0; }
            if score >= beta { return beta; }
            if score > alpha { alpha = score; }
        }
        if legal == 0 {
            return if in_check(pos) { -MATE + ply } else { 0 }; // mated sooner is worse; stalemate is a draw
        }
        alpha
    }

    fn root(&mut self, pos: &Position, depth: i32) -> (Option<Move>, i32) {
        let mut moves = MoveList::new();
        generate_moves(pos, &mut moves);
        let mut scores = [0i32; 256];
        score_moves(pos, &moves, &mut scores);

        let mut alpha = -INF;
        let mut best = None;
        for i in 0..moves.len {
            pick_next(&mut moves, &mut scores, i);
            let m = moves.moves[i];
            let Some(next) = legal_next(pos, m) else { continue };
            let score = -self.negamax(&next, depth - 1, 1, -INF, -alpha);
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

pub fn search(pos: &Position, limits: &Limits, stop: Arc<AtomicBool>) -> Option<Move> {
    let start = Instant::now();
    let mut s = Searcher {
        stop,
        deadline: time_budget(limits, pos.color_to_move).map(|d| start + d),
        nodes: 0,
        stopped: false,
    };

    let mut best = None;
    for depth in 1..=limits.depth.unwrap_or(64) as i32 {
        let (m, score) = s.root(pos, depth);
        if s.stopped { break; } // discard the unfinished iteration
        best = m;
        let ms = start.elapsed().as_millis().max(1);
        println!(
            "info depth {depth} score {} nodes {} nps {} time {ms}",
            score_string(score), s.nodes, s.nodes as u128 * 1000 / ms
        );
        if m.is_none() || score > MATE - 256 { break; } // no legal moves, or mate found
    }
    // if even depth 1 was cut off, still return something legal
    best.or_else(|| legal_moves(pos).as_slice().first().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn best(fen: &str, depth: u32) -> String {
        let pos = Position::from_fen(fen).unwrap();
        let limits = Limits { depth: Some(depth), ..Default::default() };
        search(&pos, &limits, Arc::new(AtomicBool::new(false))).unwrap().to_string()
    }

    #[test]
    fn finds_back_rank_mate() {
        assert_eq!(best("6k1/5ppp/8/8/8/8/8/R3K3 w - - 0 1", 3), "a1a8");
    }

    #[test]
    fn takes_a_free_queen() {
        assert_eq!(best("4k3/8/8/3q4/8/8/8/3QK3 w - - 0 1", 3), "d1d5");
    }
}