use crate::bitboard::Bitboard;
use crate::types::{Color, ColorMap, Square, SquareMap};

pub const NOT_A: u64 = 0xfefe_fefe_fefe_fefe;
pub const NOT_H: u64 = 0x7f7f_7f7f_7f7f_7f7f;
pub const NOT_AB: u64 = 0xfcfc_fcfc_fcfc_fcfc;
pub const NOT_GH: u64 = 0x3f3f_3f3f_3f3f_3f3f;

const fn knight_from(sq: usize) -> u64 {
    let b = 1u64 << sq;
    ((b << 17) & NOT_A)     // up 2, right 1
  | ((b << 15) & NOT_H)     // up 2, left 1
  | ((b << 10) & NOT_AB)    // up 1, right 2
  | ((b << 6)  & NOT_GH)    // up 1, left 2
  | ((b >> 17) & NOT_H)     // down 2, left 1
  | ((b >> 15) & NOT_A)     // down 2, right 1
  | ((b >> 10) & NOT_GH)    // down 1, left 2
  | ((b >> 6)  & NOT_AB)    // down 1, right 2
}

const fn king_from(sq: usize) -> u64 {
    let b = 1u64 << sq;
    (b << 8)                // up
  | (b >> 8)                // down
  | ((b << 1) & NOT_A)      // right
  | ((b >> 1) & NOT_H)      // left
  | ((b << 9) & NOT_A)      // up-right
  | ((b << 7) & NOT_H)      // up-left
  | ((b >> 7) & NOT_A)      // down-right
  | ((b >> 9) & NOT_H)      // down-left
}

const fn pawn_from(sq: usize, color: usize) -> u64 {
    let b = 1u64 << sq;
    if color == 0 {
        ((b << 9) & NOT_A) | ((b << 7) & NOT_H)
    } else {
        ((b >> 7) & NOT_A) | ((b >> 9) & NOT_H)
    }
}

const fn build_knight_table() -> [Bitboard; 64] {
    let mut table = [Bitboard::EMPTY; 64];
    let mut sq = 0;
    while sq < 64 {
        table[sq] = Bitboard(knight_from(sq));
        sq += 1;
    }
    table
}

const fn build_king_table() -> [Bitboard; 64] {
    let mut table = [Bitboard::EMPTY; 64];
    let mut sq = 0;
    while sq < 64 {
        table[sq] = Bitboard(king_from(sq));
        sq += 1;
    }
    table
}

const fn build_pawn_table(color: usize) -> [Bitboard; 64] {
    let mut table = [Bitboard::EMPTY; 64];
    let mut sq = 0;
    while sq < 64 {
        table[sq] = Bitboard(pawn_from(sq, color));
        sq += 1;
    }
    table
}

static KNIGHT_ATTACKS: SquareMap<Bitboard> = SquareMap(build_knight_table());
static KING_ATTACKS: SquareMap<Bitboard> = SquareMap(build_king_table());
static PAWN_ATTACKS: ColorMap<SquareMap<Bitboard>> = ColorMap([
    SquareMap(build_pawn_table(0)),
    SquareMap(build_pawn_table(1)),
]);

pub fn knight_attacks(sq: Square) -> Bitboard { KNIGHT_ATTACKS[sq] }
pub fn king_attacks(sq: Square) -> Bitboard { KING_ATTACKS[sq] }
pub fn pawn_attacks(sq: Square, color: Color) -> Bitboard { PAWN_ATTACKS[color][sq] }

pub const ROOK_DIRS: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
pub const BISHOP_DIRS: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

pub const fn slide_attacks(sq: usize, occ: u64, dirs: &[(i8, i8); 4]) -> u64 {
    let mut result = 0u64;
    let mut d = 0;
    while d < 4 {
        let (df, dr) = dirs[d];
        let mut f = (sq & 7) as i8 + df;
        let mut r = (sq >> 3) as i8 + dr;
        while f >= 0 && f < 8 && r >= 0 && r < 8 {
            let bit = 1u64 << ((r * 8 + f) as u32);
            result |= bit;
            if occ & bit != 0 {
                break; // blocker included, nothing beyond it
            }
            f += df;
            r += dr;
        }
        d += 1;
    }
    result
}

pub fn rook_attacks_slow(sq: Square, occ: Bitboard) -> Bitboard {
    Bitboard(slide_attacks(sq.0 as usize, occ.0, &ROOK_DIRS))
}
pub fn bishop_attacks_slow(sq: Square, occ: Bitboard) -> Bitboard {
    Bitboard(slide_attacks(sq.0 as usize, occ.0, &BISHOP_DIRS))
}
pub fn queen_attacks_slow(sq: Square, occ: Bitboard) -> Bitboard {
    rook_attacks_slow(sq, occ) | bishop_attacks_slow(sq, occ)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_counts() {
        assert_eq!(knight_attacks(Square::new(0, 0)).count(), 2); // a1
        assert_eq!(knight_attacks(Square::new(4, 3)).count(), 8); // e4
        assert_eq!(knight_attacks(Square::new(7, 7)).count(), 2); // h8
        assert_eq!(king_attacks(Square::new(0, 0)).count(), 3);
        assert_eq!(king_attacks(Square::new(4, 3)).count(), 8);
        assert_eq!(king_attacks(Square::new(7, 7)).count(), 3);
        assert!(knight_attacks(Square::new(0, 0)).contains(Square::new(1, 2))); // a1 -> b3
        assert!(knight_attacks(Square::new(0, 0)).contains(Square::new(2, 1))); // a1 -> c2
    }

    fn reference(sq: u8, offsets: &[(i8, i8)]) -> Bitboard {
        let (f, r) = ((sq & 7) as i8, (sq >> 3) as i8);
        let mut bb = Bitboard::EMPTY;
        for &(df, dr) in offsets {
            let (nf, nr) = (f + df, r + dr);
            if (0..8).contains(&nf) && (0..8).contains(&nr) {
                bb |= Bitboard::from_square(Square::new(nf as u8, nr as u8));
            }
        }
        bb
    }

    #[test]
    fn matches_reference_on_all_squares() {
        let knight = [(1,2),(2,1),(2,-1),(1,-2),(-1,-2),(-2,-1),(-2,1),(-1,2)];
        let king = [(1,0),(1,1),(0,1),(-1,1),(-1,0),(-1,-1),(0,-1),(1,-1)];
        for sq in 0..64u8 {
            assert_eq!(knight_attacks(Square(sq)), reference(sq, &knight), "knight sq {}", sq);
            assert_eq!(king_attacks(Square(sq)), reference(sq, &king), "king sq {}", sq);
        }
    }

    #[test]
    fn pawn_known() {
        // white pawn on e4 attacks d5 and f5
        let w = pawn_attacks(Square::new(4, 3), Color::White);
        assert_eq!(w.count(), 2);
        assert!(w.contains(Square::new(3, 4)) && w.contains(Square::new(5, 4)));
        // black pawn on e4 attacks d3 and f3
        let b = pawn_attacks(Square::new(4, 3), Color::Black);
        assert!(b.contains(Square::new(3, 2)) && b.contains(Square::new(5, 2)));
        // edge: a-file pawn has one target, last-rank pawn has none
        assert_eq!(pawn_attacks(Square::new(0, 3), Color::White).count(), 1);
        assert_eq!(pawn_attacks(Square::new(4, 7), Color::White).count(), 0);
    }

    #[test]
    fn pawns_match_reference() {
        let white = [(1, 1), (-1, 1)];
        let black = [(1, -1), (-1, -1)];
        for sq in 0..64u8 {
            assert_eq!(pawn_attacks(Square(sq), Color::White), reference(sq, &white), "white sq {}", sq);
            assert_eq!(pawn_attacks(Square(sq), Color::Black), reference(sq, &black), "black sq {}", sq);
        }
    }

    #[test]
    fn slider_counts_on_empty_board() {
        let empty = Bitboard::EMPTY;
        assert_eq!(rook_attacks_slow(Square::new(0, 0), empty).count(), 14);
        assert_eq!(rook_attacks_slow(Square::new(4, 3), empty).count(), 14);
        assert_eq!(bishop_attacks_slow(Square::new(0, 0), empty).count(), 7);
        assert_eq!(bishop_attacks_slow(Square::new(4, 3), empty).count(), 13);
        assert_eq!(queen_attacks_slow(Square::new(4, 3), empty).count(), 27);
    }

    #[test]
    fn rook_stops_at_blockers() {
        // rook a1, blockers on a4 and c1
        let occ = Bitboard::from_square(Square::new(0, 3)) | Bitboard::from_square(Square::new(2, 0));
        let att = rook_attacks_slow(Square::new(0, 0), occ);
        // a2 a3 a4 (blocker included) and b1 c1 (blocker included) = 5
        assert_eq!(att.count(), 5);
        assert!(att.contains(Square::new(0, 3)));   // a4: capturable
        assert!(!att.contains(Square::new(0, 4)));  // a5: hidden behind it
        assert!(!att.contains(Square::new(3, 0)));  // d1: hidden behind c1
    }

    #[test]
    fn bishop_stops_at_blockers() {
        // bishop c1, blocker on e3
        let occ = Bitboard::from_square(Square::new(4, 2));
        let att = bishop_attacks_slow(Square::new(2, 0), occ);
        assert!(att.contains(Square::new(4, 2)));   // e3
        assert!(!att.contains(Square::new(5, 3)));  // f4 hidden
        assert!(att.contains(Square::new(0, 2)));   // a3, the other diagonal
    }
} // mod tests