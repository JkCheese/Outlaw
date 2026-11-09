use crate::attacks::*;
use crate::bitboard::Bitboard;
use crate::moves::{Move, MoveList};
use crate::position::*;
use crate::sliders::{bishop_attacks, rook_attacks, queen_attacks};
use crate::types::*;

pub fn generate_moves(pos: &Position, moves: &mut MoveList) {
    let me = pos.color_to_move;
    let own = pos.occupied_by[me];
    let occ = pos.occupied;

    for from in pos.pieces[me][Piece::Knight] {
        for to in knight_attacks(from) & !own { moves.push(Move::new(from, to)); }
    }
    for from in pos.pieces[me][Piece::Bishop] {
        for to in bishop_attacks(from, occ) & !own { moves.push(Move::new(from, to)); }
    }
    for from in pos.pieces[me][Piece::Rook] {
        for to in rook_attacks(from, occ) & !own { moves.push(Move::new(from, to)); }
    }
    for from in pos.pieces[me][Piece::Queen] {
        for to in queen_attacks(from, occ) & !own { moves.push(Move::new(from, to)); }
    }
    for from in pos.pieces[me][Piece::King] {
        for to in king_attacks(from) & !own { moves.push(Move::new(from, to)); }
    }

    generate_pawn_moves(pos, moves);
    generate_castling(pos, moves);
}

/// Only the "noisy" moves quiescence search cares about: captures (en passant included) and
/// promotions (quiet pushes to the last rank too). Same pseudo-legal rules as `generate_moves`,
/// so callers still filter with `legal_next`. Skipping quiet moves entirely is what makes
/// quiescence nodes cheap: most positions have ~30 quiet moves and only a handful of captures.
pub fn generate_captures(pos: &Position, moves: &mut MoveList) {
    let me = pos.color_to_move;
    let enemy = pos.occupied_by[me.flip()];
    let occ = pos.occupied;

    for from in pos.pieces[me][Piece::Knight] {
        for to in knight_attacks(from) & enemy { moves.push(Move::new(from, to)); }
    }
    for from in pos.pieces[me][Piece::Bishop] {
        for to in bishop_attacks(from, occ) & enemy { moves.push(Move::new(from, to)); }
    }
    for from in pos.pieces[me][Piece::Rook] {
        for to in rook_attacks(from, occ) & enemy { moves.push(Move::new(from, to)); }
    }
    for from in pos.pieces[me][Piece::Queen] {
        for to in queen_attacks(from, occ) & enemy { moves.push(Move::new(from, to)); }
    }
    for from in pos.pieces[me][Piece::King] {
        for to in king_attacks(from) & enemy { moves.push(Move::new(from, to)); }
    }

    let (dir, promo_rank): (i8, u8) = if me == Color::White { (8, 7) } else { (-8, 0) };
    for from in pos.pieces[me][Piece::Pawn] {
        let attacks = pawn_attacks(from, me);

        // a push is "noisy" only if it promotes
        let one = Square((from.0 as i8 + dir) as u8);
        if one.rank() == promo_rank && !occ.contains(one) {
            push_pawn_move(moves, from, one, promo_rank);
        }
        for to in attacks & enemy {
            push_pawn_move(moves, from, to, promo_rank);
        }
        if let Some(ep) = pos.ep_sq {
            if attacks.contains(ep) {
                moves.push(Move::special(from, ep, Move::EN_PASSANT));
            }
        }
    }
}

fn push_pawn_move(moves: &mut MoveList, from: Square, to: Square, promo_rank: u8) {
    if to.rank() == promo_rank {
        for piece in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight] {
            moves.push(Move::promotion(from, to, piece));
        }
    } else {
        moves.push(Move::new(from, to));
    }
}

pub fn generate_pawn_moves(pos: &Position, moves: &mut MoveList) {
    let us = pos.color_to_move;
    let them = us.flip();
    let (dir, start_rank, promo_rank): (i8, u8, u8) =
        if us == Color::White { (8, 1, 7) } else { (-8, 6, 0) };
    let enemies = pos.occupied_by[them];

    for from in pos.pieces[us][Piece::Pawn] {
        let attacks = pawn_attacks(from, us);

        // pushes
        let one = Square((from.0 as i8 + dir) as u8);
        if !pos.occupied.contains(one) {
            push_pawn_move(moves, from, one, promo_rank);
            if from.rank() == start_rank {
                let two = Square((from.0 as i8 + 2 * dir) as u8);
                if !pos.occupied.contains(two) {
                    moves.push(Move::new(from, two));
                }
            }
        }

        // captures
        for to in attacks & enemies {
            push_pawn_move(moves, from, to, promo_rank);
        }

        // en passant
        if let Some(ep) = pos.ep_sq {
            if attacks.contains(ep) {
                moves.push(Move::special(from, ep, Move::EN_PASSANT));
            }
        }
    }
}

fn span(a: Square, b: Square) -> Bitboard {
    let (lo, hi) = if a.0 <= b.0 { (a.0, b.0) } else { (b.0, a.0) };
    Bitboard((2u64 << hi).wrapping_sub(1u64 << lo))
}

fn generate_castling(pos: &Position, moves: &mut MoveList) {
    let us = pos.color_to_move;
    let them = us.flip();
    let king_bb = pos.pieces[us][Piece::King];
    if king_bb.is_empty() { return; }
    let king_from = king_bb.lsb();
    let rank = king_from.rank();

    let rights: [(u8, bool); 2] = if us == Color::White {
        [(CASTLE_WK, true), (CASTLE_WQ, false)]
    } else {
        [(CASTLE_BK, true), (CASTLE_BQ, false)]
    };

    for (bit, kingside) in rights {
        if pos.castling_rights & bit == 0 { continue; }

        let rook_from = pos.castling_rook_sqs[bit.trailing_zeros() as usize];
        let (king_file, rook_file) = if kingside { (6, 5) } else { (2, 3) };
        let king_to = Square::new(king_file, rank);
        let rook_to = Square::new(rook_file, rank);

        // 1. everything either piece crosses or lands on must be empty (except the two movers)
        let king_path = span(king_from, king_to);
        let rook_path = span(rook_from, rook_to);
        let movers = Bitboard::from_square(king_from) | Bitboard::from_square(rook_from);
        if !(pos.occupied & (king_path | rook_path) & !movers).is_empty() { continue; }

        // 2. every square on the king's path must be safe, with the rook lifted off the board
        let occ = pos.occupied & !Bitboard::from_square(rook_from);
        if king_path.into_iter().any(|sq| pos.is_square_attacked_occ(sq, them, occ)) { continue; }

        moves.push(Move::special(king_from, rook_from, Move::CASTLING));
    }
}

// the position after `m`, or None if it leaves the mover's own king attacked
pub fn legal_next(pos: &Position, m: Move) -> Option<Position> {
    let next = pos.make_move(m);
    let king = next.pieces[pos.color_to_move][Piece::King].lsb();
    if next.is_square_attacked(king, next.color_to_move) { None } else { Some(next) }
}

pub fn legal_moves(pos: &Position) -> MoveList {
    let mut all = MoveList::new();
    generate_moves(pos, &mut all);
    let mut legal = MoveList::new();
    for &m in all.as_slice() {
        if legal_next(pos, m).is_some() { legal.push(m); }
    }
    legal
}

#[cfg(test)]
mod tests {
    use super::*;

    fn castles(fen: &str) -> MoveList {
        let pos = Position::from_fen(fen).unwrap();
        let mut moves = MoveList::new();
        generate_castling(&pos, &mut moves);
        moves
    }

    /// The capture generator must produce EXACTLY the moves of `generate_moves` that capture
    /// or promote, in every position of a whole move tree (not just a few hand-picked ones).
    fn check_captures(pos: &Position, depth: u32, seen: &mut (u64, u64, u64)) {
        let mut all = MoveList::new();
        generate_moves(pos, &mut all);
        let mut expected: Vec<u16> = all.as_slice().iter()
            .filter(|&&m| pos.is_capture(m) || m.promotion_piece().is_some())
            .map(|m| m.raw()).collect();
        let mut caps = MoveList::new();
        generate_captures(pos, &mut caps);
        let mut got: Vec<u16> = caps.as_slice().iter().map(|m| m.raw()).collect();
        expected.sort();
        got.sort();
        assert_eq!(got, expected, "capture list differs in\n{pos}");
        seen.0 += 1;
        seen.1 += caps.as_slice().iter().filter(|m| m.kind() == Move::EN_PASSANT).count() as u64;
        seen.2 += caps.as_slice().iter().filter(|m| m.promotion_piece().is_some()).count() as u64;
        if depth == 0 { return; }
        for &m in all.as_slice() {
            if let Some(next) = legal_next(pos, m) { check_captures(&next, depth - 1, seen); }
        }
    }

    #[test]
    fn capture_generator_matches_filtered_full_generator() {
        let mut seen = (0, 0, 0);
        for (fen, depth) in [
            ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 4),
            ("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", 3),
            ("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", 4),                       // en passant
            ("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1", 3), // promotions
            ("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8", 3),
            ("4k3/P7/8/8/8/8/p7/4K3 w - - 0 1", 3),                                 // quiet promotions, both colours
            ("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1", 2),
        ] {
            check_captures(&Position::from_fen(fen).unwrap(), depth, &mut seen);
        }
        // make sure the interesting cases were really exercised, not just the easy ones
        assert!(seen.1 > 0, "no en passant captures were seen");
        assert!(seen.2 > 0, "no promotions were seen");
        println!("checked {} positions ({} ep, {} promotion moves)", seen.0, seen.1, seen.2);
    }

    #[test]
    fn king_already_on_target() {
        let fen = "4k3/8/8/8/8/8/8/6KR w K - 0 1";
        let moves = castles(fen);
        assert_eq!(moves.len, 1);
        let after = Position::from_fen(fen).unwrap().make_move(moves.moves[0]);
        assert_eq!(after.mailbox[Square::new(6, 0)], Some((Color::White, Piece::King)));
        assert_eq!(after.mailbox[Square::new(5, 0)], Some((Color::White, Piece::Rook)));
    }

    #[test]
    fn rook_destination_must_be_empty() {
        assert!(castles("4k3/8/8/8/8/8/8/5NKR w K - 0 1").is_empty());
    }

    #[test]
    fn castling_rook_shield_is_removed() {
        assert!(castles("4k3/8/8/8/8/8/8/rR4K1 w B - 0 1").is_empty());
    }

    #[test]
    fn rook_next_to_king() {
        assert_eq!(castles("4k3/8/8/8/8/8/8/2KR4 w D - 0 1").len, 1);
    }

    #[test]
    fn standard_both_sides() {
        assert_eq!(castles("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").len, 2);
    }

} // mod tests