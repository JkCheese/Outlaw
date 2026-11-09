use crate::types::{Color, Piece, Square};

// splitmix64: a tiny deterministic generator. Because it is a `const fn`, the compiler
// runs it during the build and the finished key table is baked into the executable.
const fn splitmix(state: u64) -> (u64, u64) {
    let s = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = s;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (s, z ^ (z >> 31))
}

pub struct Keys {
    piece: [u64; 768],  // index = (color * 6 + piece) * 64 + square
    castling: [u64; 16], // index = the 4-bit castling-rights mask
    ep_file: [u64; 8],
    pub side: u64,       // XORed in when it is black's turn
}

const fn build() -> Keys {
    let mut state = 0x1234_5678_9ABC_DEF0u64; // fixed seed: same keys on every run
    let mut piece = [0u64; 768];
    let mut castling = [0u64; 16];
    let mut ep_file = [0u64; 8];

    let mut i = 0;
    while i < 768 {
        let (s, v) = splitmix(state);
        state = s;
        piece[i] = v;
        i += 1;
    }
    i = 0;
    while i < 16 {
        let (s, v) = splitmix(state);
        state = s;
        castling[i] = v;
        i += 1;
    }
    i = 0;
    while i < 8 {
        let (s, v) = splitmix(state);
        state = s;
        ep_file[i] = v;
        i += 1;
    }
    let (_, side) = splitmix(state);
    Keys { piece, castling, ep_file, side }
}

pub static KEYS: Keys = build();

#[inline(always)]
pub fn piece_key(color: Color, piece: Piece, sq: Square) -> u64 {
    KEYS.piece[(color as usize * 6 + piece as usize) * 64 + sq.0 as usize]
}

#[inline(always)]
pub fn castling_key(rights: u8) -> u64 {
    KEYS.castling[rights as usize]
}

#[inline(always)]
pub fn ep_key(sq: Square) -> u64 {
    KEYS.ep_file[sq.file() as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_nonzero_and_distinct() {
        let mut all: Vec<u64> = Vec::new();
        all.extend_from_slice(&KEYS.piece);
        all.extend_from_slice(&KEYS.castling);
        all.extend_from_slice(&KEYS.ep_file);
        all.push(KEYS.side);
        assert_eq!(all.len(), 793);
        assert!(all.iter().all(|&k| k != 0));
        all.sort();
        all.dedup();
        assert_eq!(all.len(), 793);
    }

    #[test]
    fn first_keys_match_the_worked_example() {
        assert_eq!(KEYS.piece[0], 0x1619_22c6_45ce_50e8);
        assert_eq!(KEYS.piece[1], 0xad76_0caf_a169_7b60);
        assert_eq!(KEYS.piece[2], 0x3501_ff44_902c_a50d);
    }
}