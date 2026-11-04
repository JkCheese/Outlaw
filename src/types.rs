use std::ops::{Index, IndexMut};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Color { White, Black }

impl Color {
    pub fn flip(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Piece { Pawn, Knight, Bishop, Rook, Queen, King }

impl Piece {
    pub const fn to_char(self) -> char {
        match self {
            Piece::Pawn => 'p',
            Piece::Knight => 'n',
            Piece::Bishop => 'b',
            Piece::Rook => 'r',
            Piece::Queen => 'q',
            Piece::King => 'k',
        }
    }

    pub const fn from_char(c: char) -> Option<Piece> {
        match c {
            'p' => Some(Piece::Pawn),
            'n' => Some(Piece::Knight),
            'b' => Some(Piece::Bishop),
            'r' => Some(Piece::Rook),
            'q' => Some(Piece::Queen),
            'k' => Some(Piece::King),
            _ => None
        }
    }
}

// ---- ColorMap: [T; 2] indexed by Color ----
#[derive(Copy, Clone, Debug)]
pub struct ColorMap<T>(pub [T; 2]);

impl<T: Copy> ColorMap<T> {
    pub fn new(value: T) -> Self { ColorMap([value; 2]) }
}
impl<T> Index<Color> for ColorMap<T> {
    type Output = T;
    fn index(&self, c: Color) -> &T { &self.0[c as usize] }
}
impl<T> IndexMut<Color> for ColorMap<T> {
    fn index_mut(&mut self, c: Color) -> &mut T { &mut self.0[c as usize] }
}

// ---- PieceMap: [T; 6] indexed by Piece ----
#[derive(Copy, Clone, Debug)]
pub struct PieceMap<T>(pub [T; 6]);

impl<T: Copy> PieceMap<T> {
    pub fn new(value: T) -> Self { PieceMap([value; 6]) }
}
impl<T> Index<Piece> for PieceMap<T> {
    type Output = T;
    fn index(&self, p: Piece) -> &T { &self.0[p as usize] }
}
impl<T> IndexMut<Piece> for PieceMap<T> {
    fn index_mut(&mut self, p: Piece) -> &mut T { &mut self.0[p as usize] }
}

// ---- SquareMap: [T; 64] indexed by Square ----
#[derive(Copy, Clone, Debug)]
pub struct SquareMap<T>(pub [T; 64]);

impl<T: Copy> SquareMap<T> {
    pub fn new(value: T) -> Self { SquareMap([value; 64]) }
}
impl<T> Index<Square> for SquareMap<T> {
    type Output = T;
    fn index(&self, sq: Square) -> &T { &self.0[sq.0 as usize] }
}
impl<T> IndexMut<Square> for SquareMap<T> {
    fn index_mut(&mut self, sq: Square) -> &mut T { &mut self.0[sq.0 as usize] }
}

/// Squares are 0..=63 with a1 = 0, b1 = 1, ... h8 = 63.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Square(pub u8);

impl Square {
    pub const fn new(file: u8, rank: u8) -> Square {
        Square(rank * 8 + file)
    }
    pub const fn file(self) -> u8 {
        self.0 & 7
    }
    pub const fn rank(self) -> u8 {
        self.0 >> 3
    }
}