use std::fmt;
use crate::types::{Piece, Square};

// bits 0-5: from | 6-11: to | 12-13: promotion piece | 14-15: kind
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Move(u16);

impl Move {
    pub const NORMAL: u16 = 0;
    pub const PROMOTION: u16 = 1;
    pub const EN_PASSANT: u16 = 2;
    pub const CASTLING: u16 = 3;

    pub const fn new(from: Square, to: Square) -> Self {
        Move((from.0 as u16) | ((to.0 as u16) << 6))
    }

    pub const fn special(from: Square, to: Square, kind: u16) -> Self {
        debug_assert!(kind == Move::EN_PASSANT || kind == Move::CASTLING, "bad special kind");
        Move((from.0 as u16) | ((to.0 as u16) << 6) | (kind << 14))
    }

    pub const fn promotion(from: Square, to: Square, piece: Piece) -> Self {
        debug_assert!(piece as u16 >= 1 && piece as u16 <= 4, "bad promotion piece");
        Move(
            (from.0 as u16)
          | ((to.0 as u16) << 6)
          | (((piece as u16) - 1) << 12) // knight = 1 -> 0 ... queen = 4 -> 3
          | (Move::PROMOTION << 14)
        )
    }

    pub const fn raw(self) -> u16 { self.0 }
    pub const fn from_sq(self) -> Square { Square((self.0 & 0b111111) as u8) }
    pub const fn to_sq(self) -> Square { Square(((self.0 >> 6) & 0b111111) as u8) }
    pub const fn from_to(self) -> usize { (self.0 & 0xFFF) as usize }
    pub const fn kind(self) -> u16 { self.0 >> 14 }

    pub const fn promotion_piece(self) -> Option<Piece> {
        if self.kind() != Move::PROMOTION {
            return None;
        }
        Some(match (self.0 >> 12) & 3 {
            0 => Piece::Knight,
            1 => Piece::Bishop,
            2 => Piece::Rook,
            _ => Piece::Queen,
        })
    }
} // impl Move

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let from = self.from_sq();
        let to = self.to_sq();
        write!(
            f, "{}{}{}{}",
            (b'a' + from.file()) as char, from.rank() + 1,
            (b'a' + to.file()) as char, to.rank() + 1,
        )?;
        if let Some(p) = self.promotion_piece() {
            write!(f, "{}", p.to_char())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_move_roundtrip() {
        let m = Move::new(Square::new(4, 1), Square::new(4, 3));
        assert_eq!(m.from_sq(), Square::new(4, 1));
        assert_eq!(m.to_sq(), Square::new(4, 3));
        assert_eq!(m.kind(), Move::NORMAL);
        assert_eq!(m.promotion_piece(), None);
        assert_eq!(m.to_string(), "e2e4");
    }

    #[test]
    fn all_promotion_pieces_roundtrip() {
        for piece in [Piece::Knight, Piece::Bishop, Piece::Rook, Piece::Queen] {
            let m = Move::promotion(Square::new(0, 6), Square::new(0, 7), piece);
            assert_eq!(m.kind(), Move::PROMOTION);
            assert_eq!(m.promotion_piece(), Some(piece));
            assert_eq!(m.from_sq(), Square::new(0, 6));
            assert_eq!(m.to_sq(), Square::new(0, 7));
        }
        let q = Move::promotion(Square::new(0, 6), Square::new(0, 7), Piece::Queen);
        assert_eq!(q.to_string(), "a7a8q");
    }

    #[test]
    fn special_kinds() {
        let ep = Move::special(Square::new(4, 4), Square::new(3, 5), Move::EN_PASSANT);
        assert_eq!(ep.kind(), Move::EN_PASSANT);
        assert_eq!(ep.promotion_piece(), None);
        let castle = Move::special(Square::new(4, 0), Square::new(6, 0), Move::CASTLING);
        assert_eq!(castle.kind(), Move::CASTLING);
    }
} // mod tests