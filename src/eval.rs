use crate::position::Position;
use crate::types::Color;

// indexed by `piece as usize`: pawn, knight, bishop, rook, queen, king
pub const VALUE: [i32; 6] = [100, 320, 330, 500, 900, 0];

pub static MG_VALUE: [i32; 6] = [ 82, 337, 365, 477, 1025, 0 ];
pub static EG_VALUE: [i32; 6] = [ 94, 281, 297, 512,  936, 0 ];

pub static MG_PAWN_TABLE: [i32; 64] = [
      0,   0,   0,   0,   0,   0,  0,   0,
     98, 134,  61,  95,  68, 126, 34, -11,
     -6,   7,  26,  31,  65,  56, 25, -20,
    -14,  13,   6,  21,  23,  12, 17, -23,
    -27,  -2,  -5,  12,  17,   6, 10, -25,
    -26,  -4,  -4, -10,   3,   3, 33, -12,
    -35,  -1, -20, -23, -15,  24, 38, -22,
      0,   0,   0,   0,   0,   0,  0,   0,
];

pub static EG_PAWN_TABLE: [i32; 64] = [
      0,   0,   0,   0,   0,   0,   0,   0,
    178, 173, 158, 134, 147, 132, 165, 187,
     94, 100,  85,  67,  56,  53,  82,  84,
     32,  24,  13,   5,  -2,   4,  17,  17,
     13,   9,  -3,  -7,  -7,  -8,   3,  -1,
      4,   7,  -6,   1,   0,  -5,  -1,  -8,
     13,   8,   8,  10,  13,   0,   2,  -7,
      0,   0,   0,   0,   0,   0,   0,   0,
];

pub static MG_KNIGHT_TABLE: [i32; 64] = [
    -167, -89, -34, -49,  61, -97, -15, -107,
     -73, -41,  72,  36,  23,  62,   7,  -17,
     -47,  60,  37,  65,  84, 129,  73,   44,
      -9,  17,  19,  53,  37,  69,  18,   22,
     -13,   4,  16,  13,  28,  19,  21,   -8,
     -23,  -9,  12,  10,  19,  17,  25,  -16,
     -29, -53, -12,  -3,  -1,  18, -14,  -19,
    -105, -21, -58, -33, -17, -28, -19,  -23,
];

pub static EG_KNIGHT_TABLE: [i32; 64] = [
    -58, -38, -13, -28, -31, -27, -63, -99,
    -25,  -8, -25,  -2,  -9, -25, -24, -52,
    -24, -20,  10,   9,  -1,  -9, -19, -41,
    -17,   3,  22,  22,  22,  11,   8, -18,
    -18,  -6,  16,  25,  16,  17,   4, -18,
    -23,  -3,  -1,  15,  10,  -3, -20, -22,
    -42, -20, -10,  -5,  -2, -20, -23, -44,
    -29, -51, -23, -15, -22, -18, -50, -64,
];

pub static MG_BISHOP_TABLE: [i32; 64] = [
    -29,   4, -82, -37, -25, -42,   7,  -8,
    -26,  16, -18, -13,  30,  59,  18, -47,
    -16,  37,  43,  40,  35,  50,  37,  -2,
     -4,   5,  19,  50,  37,  37,   7,  -2,
     -6,  13,  13,  26,  34,  12,  10,   4,
      0,  15,  15,  15,  14,  27,  18,  10,
      4,  15,  16,   0,   7,  21,  33,   1,
    -33,  -3, -14, -21, -13, -12, -39, -21,
];

pub static EG_BISHOP_TABLE: [i32; 64] = [
    -14, -21, -11,  -8, -7,  -9, -17, -24,
     -8,  -4,   7, -12, -3, -13,  -4, -14,
      2,  -8,   0,  -1, -2,   6,   0,   4,
     -3,   9,  12,   9, 14,  10,   3,   2,
     -6,   3,  13,  19,  7,  10,  -3,  -9,
    -12,  -3,   8,  10, 13,   3,  -7, -15,
    -14, -18,  -7,  -1,  4,  -9, -15, -27,
    -23,  -9, -23,  -5, -9, -16,  -5, -17,
];

pub static MG_ROOK_TABLE: [i32; 64] = [
     32,  42,  32,  51, 63,  9,  31,  43,
     27,  32,  58,  62, 80, 67,  26,  44,
     -5,  19,  26,  36, 17, 45,  61,  16,
    -24, -11,   7,  26, 24, 35,  -8, -20,
    -36, -26, -12,  -1,  9, -7,   6, -23,
    -45, -25, -16, -17,  3,  0,  -5, -33,
    -44, -16, -20,  -9, -1, 11,  -6, -71,
    -19, -13,   1,  17, 16,  7, -37, -26,
];

pub static EG_ROOK_TABLE: [i32; 64] = [
    13, 10, 18, 15, 12,  12,   8,   5,
    11, 13, 13, 11, -3,   3,   8,   3,
     7,  7,  7,  5,  4,  -3,  -5,  -3,
     4,  3, 13,  1,  2,   1,  -1,   2,
     3,  5,  8,  4, -5,  -6,  -8, -11,
    -4,  0, -5, -1, -7, -12,  -8, -16,
    -6, -6,  0,  2, -9,  -9, -11,  -3,
    -9,  2,  3, -1, -5, -13,   4, -20,
];

pub static MG_QUEEN_TABLE: [i32; 64] = [
    -28,   0,  29,  12,  59,  44,  43,  45,
    -24, -39,  -5,   1, -16,  57,  28,  54,
    -13, -17,   7,   8,  29,  56,  47,  57,
    -27, -27, -16, -16,  -1,  17,  -2,   1,
     -9, -26,  -9, -10,  -2,  -4,   3,  -3,
    -14,   2, -11,  -2,  -5,   2,  14,   5,
    -35,  -8,  11,   2,   8,  15,  -3,   1,
     -1, -18,  -9,  10, -15, -25, -31, -50,
];

pub static EG_QUEEN_TABLE: [i32; 64] = [
     -9,  22,  22,  27,  27,  19,  10,  20,
    -17,  20,  32,  41,  58,  25,  30,   0,
    -20,   6,   9,  49,  47,  35,  19,   9,
      3,  22,  24,  45,  57,  40,  57,  36,
    -18,  28,  19,  47,  31,  34,  39,  23,
    -16, -27,  15,   6,   9,  17,  10,   5,
    -22, -23, -30, -16, -16, -23, -36, -32,
    -33, -28, -22, -43,  -5, -32, -20, -41,
];

pub static MG_KING_TABLE: [i32; 64] = [
    -65,  23,  16, -15, -56, -34,   2,  13,
     29,  -1, -20,  -7,  -8,  -4, -38, -29,
     -9,  24,   2, -16, -20,   6,  22, -22,
    -17, -20, -12, -27, -30, -25, -14, -36,
    -49,  -1, -27, -39, -46, -44, -33, -51,
    -14, -14, -22, -46, -44, -30, -15, -27,
      1,   7,  -8, -64, -43, -16,   9,   8,
    -15,  36,  12, -54,   8, -28,  24,  14,
];

pub static EG_KING_TABLE: [i32; 64] = [
    -74, -35, -18, -18, -11,  15,   4, -17,
    -12,  17,  14,  17,  17,  38,  23,  11,
     10,  17,  23,  15,  20,  45,  44,  13,
     -8,  22,  24,  27,  26,  33,  26,   3,
    -18,  -4,  21,  24,  27,  23,   9, -11,
    -19,  -3,  11,  21,  23,  16,   7,  -9,
    -27, -11,   4,  13,  14,   4,  -5, -17,
    -53, -34, -21, -11, -28, -14, -24, -43
];

// ---------------------------------------------------------------------------
// Tapered evaluation
// ---------------------------------------------------------------------------

// Tables indexed by `piece as usize` (pawn, knight, bishop, rook, queen, king).
static MG_TABLES: [&[i32; 64]; 6] = [
    &MG_PAWN_TABLE, &MG_KNIGHT_TABLE, &MG_BISHOP_TABLE, &MG_ROOK_TABLE, &MG_QUEEN_TABLE, &MG_KING_TABLE,
];
static EG_TABLES: [&[i32; 64]; 6] = [
    &EG_PAWN_TABLE, &EG_KNIGHT_TABLE, &EG_BISHOP_TABLE, &EG_ROOK_TABLE, &EG_QUEEN_TABLE, &EG_KING_TABLE,
];

// How much each piece counts toward "we are still in the middlegame".
// Full board = 4 knights/bishops (4) + 4 rooks (8) + 2 queens (8) ... = 24.
const PHASE_INC: [i32; 6] = [0, 1, 1, 2, 4, 0];
const MAX_PHASE: i32 = 24;

/// These tables are written the way a chess diagram is printed: index 0 is a8, so the
/// top row is rank 8 and White's pawns start on the second-to-last row. Our squares have
/// a1 = 0, so for a WHITE piece we flip the rank (sq ^ 56 maps rank r to rank 7 - r).
/// A BLACK piece sees the board from the other side, which is exactly the table as
/// written, so it uses `sq` unchanged.
#[inline(always)]
fn table_index(color: Color, sq: u8) -> usize {
    if color == Color::White { (sq ^ 56) as usize } else { sq as usize }
}

/// Score from the point of view of the side to move (what negamax needs).
pub fn evaluate(pos: &Position) -> i32 {
    if pos.insufficient_material() { return 0; } // dead draw: no amount of "material" matters
    let mut mg = [0i32; 2]; // [white, black]
    let mut eg = [0i32; 2];
    let mut phase = 0;

    for sq in pos.occupied {
        if let Some((color, piece)) = pos.mailbox[sq] {
            let p = piece as usize;
            let i = table_index(color, sq.0);
            mg[color as usize] += MG_VALUE[p] + MG_TABLES[p][i];
            eg[color as usize] += EG_VALUE[p] + EG_TABLES[p][i];
            phase += PHASE_INC[p];
        }
    }

    let us = pos.color_to_move as usize;
    let them = us ^ 1;
    let mg_score = mg[us] - mg[them];
    let eg_score = eg[us] - eg[them];
    let mg_phase = phase.min(MAX_PHASE); // early promotions can push it past 24
    let eg_phase = MAX_PHASE - mg_phase;
    (mg_score * mg_phase + eg_score * eg_phase) / MAX_PHASE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startpos_is_balanced() {
        assert_eq!(evaluate(&Position::startpos()), 0);
    }

    #[test]
    fn extra_queen_is_good_for_its_owner() {
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/3QK3 w - - 0 1").unwrap();
        assert!(evaluate(&pos) > 800);
        let flipped = Position::from_fen("4k3/8/8/8/8/8/8/3QK3 b - - 0 1").unwrap();
        assert!(evaluate(&flipped) < -800); // same board, other side to move
    }

    #[test]
    fn eval_is_colour_symmetric() {
        // a position and its colour-flipped mirror must score the same for the side to move
        let a = Position::from_fen("r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3").unwrap();
        let b = Position::from_fen("rnbqkb1r/pppp1ppp/5n2/4p3/4P3/2N5/PPPP1PPP/R1BQKBNR b KQkq - 2 3").unwrap();
        assert_eq!(evaluate(&a), evaluate(&b));
    }

    #[test]
    fn pawn_on_seventh_beats_pawn_on_second() {
        let far = Position::from_fen("4k3/P7/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let near = Position::from_fen("4k3/8/8/8/8/8/P7/4K3 w - - 0 1").unwrap();
        assert!(evaluate(&far) > evaluate(&near));
        // and the same for Black, which checks the other orientation
        let far_b = Position::from_fen("4k3/8/8/8/8/8/p7/4K3 b - - 0 1").unwrap();
        let near_b = Position::from_fen("4k3/p7/8/8/8/8/8/4K3 b - - 0 1").unwrap();
        assert!(evaluate(&far_b) > evaluate(&near_b));
    }

    #[test]
    fn knight_prefers_centre_in_middlegame() {
        let centre = Position::from_fen("rnbqkb1r/pppppppp/8/8/3N4/8/PPPPPPPP/RNBQKB1R w - - 0 1").unwrap();
        let corner = Position::from_fen("rnbqkb1r/pppppppp/8/8/8/8/PPPPPPPP/RNBQKB1N w - - 0 1").unwrap();
        assert!(evaluate(&centre) > evaluate(&corner));
    }
}