use std::sync::LazyLock;

use crate::attacks::{BISHOP_DIRS, ROOK_DIRS, slide_attacks};
use crate::bitboard::Bitboard;
use crate::types::Square;

// ---------------------------------------------------------------------------
// PEXT (hardware only; the build must enable BMI2 via .cargo/config.toml)
// ---------------------------------------------------------------------------

#[cfg(not(all(target_arch = "x86_64", target_feature = "bmi2")))]
compile_error!("sliders.rs needs BMI2: build with -C target-cpu=native (see .cargo/config.toml)");

/// Gather the bits of `value` selected by `mask` into the low bits of the result,
/// keeping their order. Example: value = 0b1011_0110, mask = 0b0110_1100 -> 0b0101.
#[allow(unused_unsafe)] // newer compilers treat this call as safe when the feature is on
#[inline(always)]
pub fn pext(value: u64, mask: u64) -> u64 {
    // SAFETY: compile_error! above guarantees the bmi2 target feature is enabled.
    unsafe { core::arch::x86_64::_pext_u64(value, mask) }
}

// ---------------------------------------------------------------------------
// Blocker masks
// ---------------------------------------------------------------------------

/// Squares along the given directions from `sq`, EXCLUDING the final square of each ray.
///
/// Careful: this is NOT "all rays minus the board edges". A rook on the a-file still
/// needs its whole a-file ray (up to the 7th rank); only the square at the very end of
/// each ray is dropped. We therefore keep a square only if the NEXT square in that
/// direction is still on the board.
const fn relevant_mask(sq: usize, dirs: &[(i8, i8); 4]) -> u64 {
    let mut mask = 0u64;
    let mut d = 0;
    while d < 4 {
        let (df, dr) = dirs[d];
        let mut f = (sq & 7) as i8 + df;
        let mut r = (sq >> 3) as i8 + dr;
        while f + df >= 0 && f + df < 8 && r + dr >= 0 && r + dr < 8 {
            mask |= 1u64 << (r * 8 + f);
            f += df;
            r += dr;
        }
        d += 1;
    }
    mask
}

// ---------------------------------------------------------------------------
// The table
// ---------------------------------------------------------------------------

#[derive(Copy, Clone)]
struct Entry {
    mask: u64,
    offset: usize,
}

pub struct SliderTable {
    entries: [Entry; 64],
    attacks: Vec<Bitboard>,
}

impl SliderTable {
    fn build(dirs: &[(i8, i8); 4]) -> SliderTable {
        let mut entries = [Entry { mask: 0, offset: 0 }; 64];
        let mut attacks: Vec<Bitboard> = Vec::new();

        for sq in 0..64usize {
            let mask = relevant_mask(sq, dirs);
            let offset = attacks.len();
            entries[sq] = Entry { mask, offset };
            attacks.resize(offset + (1usize << mask.count_ones()), Bitboard::EMPTY);

            // Visit EVERY subset of `mask` ("Carry-Rippler" trick: subtracting the mask
            // and re-masking steps through all subsets, ending back at 0).
            let mut subset = 0u64;
            loop {
                let index = pext(subset, mask) as usize;
                attacks[offset + index] = Bitboard(slide_attacks(sq, subset, dirs));
                subset = subset.wrapping_sub(mask) & mask;
                if subset == 0 {
                    break;
                }
            }
        }
        SliderTable { entries, attacks }
    }

    #[inline(always)]
    fn get(&self, sq: Square, occ: Bitboard) -> Bitboard {
        let e = &self.entries[sq.0 as usize];
        self.attacks[e.offset + pext(occ.0, e.mask) as usize]
    }
}

// Built the first time they are used (thread-safe, exactly once).
static ROOK_TABLE: LazyLock<SliderTable> = LazyLock::new(|| SliderTable::build(&ROOK_DIRS));
static BISHOP_TABLE: LazyLock<SliderTable> = LazyLock::new(|| SliderTable::build(&BISHOP_DIRS));

/// Call once at startup so table building doesn't happen in the middle of a search.
pub fn init() {
    LazyLock::force(&ROOK_TABLE);
    LazyLock::force(&BISHOP_TABLE);
}

#[inline(always)]
pub fn rook_attacks(sq: Square, occ: Bitboard) -> Bitboard {
    ROOK_TABLE.get(sq, occ)
}

#[inline(always)]
pub fn bishop_attacks(sq: Square, occ: Bitboard) -> Bitboard {
    BISHOP_TABLE.get(sq, occ)
}

#[inline(always)]
pub fn queen_attacks(sq: Square, occ: Bitboard) -> Bitboard {
    rook_attacks(sq, occ) | bishop_attacks(sq, occ)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attacks::{bishop_attacks_slow, queen_attacks_slow, rook_attacks_slow};

    /// Tiny deterministic PRNG so the tests need no external crate.
    struct XorShift(u64);
    impl XorShift {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    #[test]
    fn pext_worked_example() {
        assert_eq!(pext(0b1011_0110, 0b0110_1100), 0b0101);
        assert_eq!(pext(u64::MAX, 0), 0);
        assert_eq!(pext(u64::MAX, u64::MAX), u64::MAX);
    }

    #[test]
    fn mask_sizes_and_table_sizes() {
        // classic known numbers: corner rook 12 bits, d4 rook 10; corner bishop 6, d4 bishop 9
        assert_eq!(relevant_mask(0, &ROOK_DIRS).count_ones(), 12); // a1
        assert_eq!(relevant_mask(27, &ROOK_DIRS).count_ones(), 10); // d4
        assert_eq!(relevant_mask(0, &BISHOP_DIRS).count_ones(), 6); // a1
        assert_eq!(relevant_mask(27, &BISHOP_DIRS).count_ones(), 9); // d4
        assert_eq!(ROOK_TABLE.attacks.len(), 102_400);
        assert_eq!(BISHOP_TABLE.attacks.len(), 5_248);
    }

    #[test]
    fn edge_rooks_still_see_blockers() {
        // rook a4 with a blocker on a6: must see a5, a6 but NOT a7/a8
        let a4 = Square::new(0, 3);
        let occ = Bitboard::from_square(Square::new(0, 5));
        let att = rook_attacks(a4, occ);
        assert!(att.contains(Square::new(0, 5)));
        assert!(!att.contains(Square::new(0, 6)));
        assert!(!att.contains(Square::new(0, 7)));
    }

    #[test]
    fn matches_slow_on_every_square() {
        let mut rng = XorShift(0xDEAD_BEEF_CAFE_F00D);
        for sq in 0..64u8 {
            let s = Square(sq);
            // empty, full, and many random boards with different densities
            let mut boards = vec![0u64, u64::MAX];
            for _ in 0..3000 {
                let a = rng.next();
                boards.push(a); // dense
                boards.push(a & rng.next()); // medium
                boards.push(a & rng.next() & rng.next()); // sparse
            }
            for occ in boards {
                let o = Bitboard(occ);
                assert_eq!(rook_attacks(s, o), rook_attacks_slow(s, o), "rook sq {sq} occ {occ:#x}");
                assert_eq!(bishop_attacks(s, o), bishop_attacks_slow(s, o), "bishop sq {sq} occ {occ:#x}");
                assert_eq!(queen_attacks(s, o), queen_attacks_slow(s, o), "queen sq {sq} occ {occ:#x}");
            }
        }
    }

    #[test]
    fn exhaustive_for_one_rook_square() {
        // every one of the 1024 blocker layouts for a rook on d4
        let sq = 27usize;
        let mask = relevant_mask(sq, &ROOK_DIRS);
        let mut subset = 0u64;
        let mut seen = 0;
        loop {
            let o = Bitboard(subset);
            assert_eq!(rook_attacks(Square(sq as u8), o), rook_attacks_slow(Square(sq as u8), o));
            seen += 1;
            subset = subset.wrapping_sub(mask) & mask;
            if subset == 0 {
                break;
            }
        }
        assert_eq!(seen, 1024);
    }
}