use crate::position::Position;
use crate::types::{Color, Piece, Square};

// indexed by `piece as usize`: pawn, knight, bishop, rook, queen, king
pub const VALUE: [i32; 6] = [100, 320, 330, 500, 900, 0];

fn distance_from_center(sq: Square) -> i32 {
    let df = (2 * sq.file() as i32 - 7).abs();
    let dr = (2 * sq.rank() as i32 - 7).abs();
    df.max(dr) // 1, 3, 5 or 7
}

// placeholder positional terms: swap for real piece-square tables later
fn piece_bonus(piece: Piece, color: Color, sq: Square) -> i32 {
    let centre = (7 - distance_from_center(sq)) * 4; // 0..24
    let rank = sq.rank() as i32;
    let rel_rank = if color == Color::White { rank } else { 7 - rank };
    match piece {
        Piece::Pawn => rel_rank * 6,
        Piece::Knight | Piece::Bishop => centre,
        Piece::Queen => centre / 2,
        Piece::Rook | Piece::King => 0,
    }
}

pub fn evaluate(pos: &Position) -> i32 {
    let mut score = 0; // white minus black
    for sq in pos.occupied {
        if let Some((color, piece)) = pos.mailbox[sq] {
            let v = VALUE[piece as usize] + piece_bonus(piece, color, sq);
            if color == Color::White { score += v } else { score -= v }
        }
    }
    if pos.color_to_move == Color::White { score } else { -score }
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
}