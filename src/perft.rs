use crate::movegen::generate_moves;
use crate::moves::{MoveList};
use crate::position::Position;
use crate::movegen::legal_next;

pub fn perft(pos: &Position, depth: u32) -> u64 {
    if depth == 0 { return 1; }
    let mut moves = MoveList::new();
    generate_moves(pos, &mut moves);
    let mut nodes = 0;
    for &m in moves.as_slice() {
        if let Some(next) = legal_next(pos, m) {
            nodes += if depth == 1 { 1 } else { perft(&next, depth - 1) };
        }
    }
    nodes
}

pub fn divide(pos: &Position, depth: u32) -> u64 {
    let mut moves = MoveList::new();
    generate_moves(pos, &mut moves);
    let mut total = 0;
    for &m in moves.as_slice() {
        if let Some(next) = legal_next(pos, m) {
            let n = if depth <= 1 { 1 } else { perft(&next, depth - 1) };
            println!("{m}: {n}");
            total += n;
        }
    }
    println!("total: {total}");
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(fen: &str, expected: &[u64]) {
        let pos = Position::from_fen(fen).unwrap();
        for (i, &n) in expected.iter().enumerate() {
            assert_eq!(perft(&pos, i as u32 + 1), n, "{} at depth {}", fen, i + 1);
        }
    }

    #[test]
    fn start_position() {
        check("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", &[20, 400, 8902, 197281]);
    }
    #[test]
    fn kiwipete() {
        check("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", &[48, 2039, 97862]);
    }
    #[test]
    fn position_3() {
        check("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", &[14, 191, 2812, 43238]);
    }
    #[test]
    fn position_4() {
        check("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1", &[6, 264, 9467]);
    }
    #[test]
    fn position_5() {
        check("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8", &[44, 1486, 62379]);
    }

    #[test]
    #[ignore] // slow: run with `cargo test --release -- --ignored`
    fn deep_perft() {
        check("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", &[20, 400, 8902, 197281, 4865609]);
        check("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", &[48, 2039, 97862, 4085603]);
        check("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", &[14, 191, 2812, 43238, 674624]);
        check("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1", &[6, 264, 9467, 422333]);
        check("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8", &[44, 1486, 62379, 2103487]);
    }

    fn hash_walk(pos: &Position, depth: u32) {
        assert_eq!(pos.hash, pos.compute_hash());
        if depth == 0 { return; }
        let mut moves = MoveList::new();
        generate_moves(pos, &mut moves);
        for &m in moves.as_slice() {
            if let Some(next) = legal_next(pos, m) { hash_walk(&next, depth - 1); }
        }
    }

    #[test]
    fn incremental_hash_matches_recompute() {
        for (fen, depth) in [
            ("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", 3), // castling
            ("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", 4),                            // en passant
            ("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1", 3),     // promotions
            ("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8", 3),
        ] {
            hash_walk(&Position::from_fen(fen).unwrap(), depth);
        }
    }
}