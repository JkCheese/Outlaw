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

#[cfg(test)]
mod tests {
    use super::*;

    fn castles(fen: &str) -> MoveList {
        let pos = Position::from_fen(fen).unwrap();
        let mut moves = MoveList::new();
        generate_castling(&pos, &mut moves);
        moves
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