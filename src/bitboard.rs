use crate::types::Square;
use std::fmt;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not, Shl, Shr};

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct Bitboard(pub u64);

impl Bitboard {
    pub const EMPTY: Bitboard = Bitboard(0);
    pub const FULL: Bitboard = Bitboard(!0);

    pub const fn from_square(sq: Square) -> Bitboard {
        Bitboard(1u64 << sq.0)
    }
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }
    pub const fn contains(self, sq: Square) -> bool {
        (self.0 >> sq.0) & 1 != 0
    }
    /// Lowest set bit as a square. Only call on a non-empty board.
    pub const fn lsb(self) -> Square {
        Square(self.0.trailing_zeros() as u8)
    }
    /// Removes the lowest set bit and returns its square.
    pub fn pop_lsb(&mut self) -> Square {
        let sq = self.lsb();
        self.0 &= self.0 - 1;
        sq
    }
} // impl Bitboard

// Lets you write `for sq in bb { ... }` to visit every set bit.
impl Iterator for Bitboard {
    type Item = Square;
    fn next(&mut self) -> Option<Square> {
        if self.is_empty() {
            None
        } else {
            Some(self.pop_lsb())
        }
    }
}

impl BitAnd for Bitboard {
    type Output = Bitboard;
    fn bitand(self, rhs: Bitboard) -> Bitboard { Bitboard(self.0 & rhs.0) }
}
impl BitOr for Bitboard {
    type Output = Bitboard;
    fn bitor(self, rhs: Bitboard) -> Bitboard { Bitboard(self.0 | rhs.0) }
}
impl BitXor for Bitboard {
    type Output = Bitboard;
    fn bitxor(self, rhs: Bitboard) -> Bitboard { Bitboard(self.0 ^ rhs.0) }
}
impl Not for Bitboard {
    type Output = Bitboard;
    fn not(self) -> Bitboard { Bitboard(!self.0) }
}
impl BitAndAssign for Bitboard {
    fn bitand_assign(&mut self, rhs: Bitboard) { self.0 &= rhs.0; }
}
impl BitOrAssign for Bitboard {
    fn bitor_assign(&mut self, rhs: Bitboard) { self.0 |= rhs.0; }
}
impl BitXorAssign for Bitboard {
    fn bitxor_assign(&mut self, rhs: Bitboard) { self.0 ^= rhs.0; }
}
impl Shl<u8> for Bitboard {
    type Output = Bitboard;
    fn shl(self, n: u8) -> Bitboard { Bitboard(self.0 << n) }
}
impl Shr<u8> for Bitboard {
    type Output = Bitboard;
    fn shr(self, n: u8) -> Bitboard { Bitboard(self.0 >> n) }
}

// Lets you write `println!("{}", bb)`.
impl fmt::Display for Bitboard {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for rank in (0..8).rev() {
            write!(f, "{} ", rank + 1)?;
            for file in 0..8 {
                let bit = (self.0 >> (rank * 8 + file)) & 1;
                write!(f, "{} ", if bit != 0 { '1' } else { '.' })?;
            }
            writeln!(f)?;
        }
        writeln!(f, "  a b c d e f g h")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iterates_set_bits_in_order() {
        let bb = Bitboard(0b1010_0001); // bits 0, 5, 7
        let squares: Vec<u8> = bb.map(|s| s.0).collect();
        assert_eq!(squares, vec![0, 5, 7]);
    }

    #[test]
    fn square_roundtrip() {
        let sq = Square::new(4, 1); // e2
        assert_eq!(sq.0, 12);
        assert_eq!((sq.file(), sq.rank()), (4, 1));
        assert!(Bitboard::from_square(sq).contains(sq));
    }
}