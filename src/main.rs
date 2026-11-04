#![allow(dead_code)] // silences "unused" warnings while the engine is mostly empty

mod attacks;
mod bitboard;
mod position;
mod movegen;
mod moves;
mod types;

use bitboard::Bitboard;
use position::Position;
use types::Square;

fn main() {
    let bb = Bitboard::from_square(Square::new(4, 1)) | Bitboard::from_square(Square::new(4, 3));
    println!("{}", bb);
    for sq in bb {
        println!("{:?}", sq);
    }

    println!("{}", Position::startpos());
}