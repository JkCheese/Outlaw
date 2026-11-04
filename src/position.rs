use std::fmt;
use crate::bitboard::Bitboard;
use crate::types::*;
use crate::moves::Move;

pub const CASTLE_WK: u8 = 0b0001;
pub const CASTLE_WQ: u8 = 0b0010;
pub const CASTLE_BK: u8 = 0b0100;
pub const CASTLE_BQ: u8 = 0b1000;

#[derive(Copy, Clone, Debug)]
pub struct Position {
    pub pieces: ColorMap<PieceMap<Bitboard>>,
    pub occupied: Bitboard,
    pub occupied_by: ColorMap<Bitboard>,
    pub mailbox: SquareMap<Option<(Color, Piece)>>,

    pub color_to_move: Color,
    pub ep_sq: Option<Square>,
    pub half_move_clock: u8,
    pub full_move_count: u32,
    pub castling_rights: u8
}

impl Position {
    pub fn new() -> Self {
        Position {
            pieces: ColorMap::new(PieceMap::new(Bitboard::EMPTY)),
            occupied: Bitboard::EMPTY,
            occupied_by: ColorMap::new(Bitboard::EMPTY),
            mailbox: SquareMap::new(None),
            color_to_move: Color::White,
            ep_sq: None,
            half_move_clock: 0,
            full_move_count: 1,
            castling_rights: 0
        }
    }

    pub fn place_piece(&mut self, color: Color, piece: Piece, sq: Square) {
        debug_assert!(self.mailbox[sq].is_none(), "square already occupied");
        let bb = Bitboard::from_square(sq);
        self.pieces[color][piece] |= bb;
        self.occupied |= bb;
        self.occupied_by[color] |= bb;
        self.mailbox[sq] = Some((color, piece));
    }

    pub fn remove_piece(&mut self, sq: Square) {
        if let Some((color, piece)) = self.mailbox[sq].take() {
            let bb = !Bitboard::from_square(sq);
            self.pieces[color][piece] &= bb;
            self.occupied &= bb;
            self.occupied_by[color] &= bb;
        }
    }

    pub fn startpos() -> Self {
        Position::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap()
    }

    pub fn from_fen(fen: &str) -> Result<Position, String> {
        let mut pos = Position::new();
        pos.castling_rights = 0;
        let mut fields = fen.split_whitespace();

        // 1. Piece placement
        let placement = fields.next().ok_or("missing piece placement")?;
        let mut rank = 7;
        let mut file = 0;
        for c in placement.chars() {
            match c {
                '/' => {
                    if file != 8 || rank == 0 {
                        return Err("bad rank layout".to_string());
                    }
                    rank -= 1;
                    file = 0;
                }
                '1'..='8' => {
                    file += c as u8 - b'0';
                    if file > 8 {
                        return Err("too many squares in rank".to_string());
                    }
                }
                _ => {
                    let piece = Piece::from_char(c.to_ascii_lowercase())
                        .ok_or(format!("bad piece '{}'", c))?;
                    let color = if c.is_ascii_uppercase() { Color::White } else { Color::Black };
                    if file > 7 {
                        return Err("too many squares in rank".to_string());
                    }
                    pos.place_piece(color, piece, Square::new(file, rank));
                    file += 1;
                }
            }
        }
        if rank != 0 || file != 8 {
            return Err("incomplete piece placement".to_string());
        }

        // 2. Side to move
        pos.color_to_move = match fields.next() {
            Some("w") => Color::White,
            Some("b") => Color::Black,
            _ => return Err("bad side to move".to_string()),
        };

        // 3. Castling rights
        let castling = fields.next().ok_or("missing castling field")?;
        for c in castling.chars() {
            match c {
                'K' => pos.castling_rights |= CASTLE_WK,
                'Q' => pos.castling_rights |= CASTLE_WQ,
                'k' => pos.castling_rights |= CASTLE_BK,
                'q' => pos.castling_rights |= CASTLE_BQ,
                '-' => {}
                _ => return Err(format!("bad castling char '{}'", c)),
            }
        }

        // 4. En passant square
        let ep = fields.next().ok_or("missing en passant field")?;
        pos.ep_sq = if ep == "-" {
            None
        } else {
            let b: &[u8] = ep.as_bytes();
            if b.len() != 2 {
                return Err("bad en passant square".to_string());
            }
            let f = b[0].wrapping_sub(b'a');
            let r = b[1].wrapping_sub(b'1');
            if f > 7 || r > 7 {
                return Err("bad en passant square".to_string());
            }
            Some(Square::new(f, r))
        };

        // 5 and 6. Clocks (some FENs omit them, so default to 0 and 1)
        pos.half_move_clock = fields.next().unwrap_or("0").parse().map_err(|_| "bad halfmove clock")?;
        pos.full_move_count = fields.next().unwrap_or("1").parse().map_err(|_| "bad fullmove number")?;

        Ok(pos)
    } // fn from_fen()

    pub fn is_capture(&self, m: Move) -> bool {
        m.kind() == Move::EN_PASSANT || self.mailbox[m.to_sq()].is_some()
    }
} // impl Position

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for rank in (0..8).rev() {
            write!(f, "{} ", rank + 1)?;
            for file in 0..8 {
                let sq = Square::new(file, rank);
                let c = match self.mailbox[sq] {
                    Some((Color::White, piece)) => piece.to_char().to_ascii_uppercase(),
                    Some((Color::Black, piece)) => piece.to_char(),
                    None => '.',
                };
                write!(f, "{} ", c)?;
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
    fn place_and_remove() {
        let mut pos = Position::new();
        let sq = Square::new(4, 1);
        pos.place_piece(Color::White, Piece::Pawn, sq);
        assert!(pos.occupied.contains(sq));
        assert!(pos.occupied_by[Color::White].contains(sq));
        assert!(pos.pieces[Color::White][Piece::Pawn].contains(sq));
        assert_eq!(pos.mailbox[sq], Some((Color::White, Piece::Pawn)));

        pos.remove_piece(sq);
        assert!(pos.occupied.is_empty());
        assert!(pos.mailbox[sq].is_none());
    }

    #[test]
    fn startpos_has_32_pieces() {
        let pos = Position::startpos();
        assert_eq!(pos.occupied.count(), 32);
        assert_eq!(pos.occupied_by[Color::White].count(), 16);
    }

    #[test]
    fn fen_startpos_matches() {
        let a = Position::startpos();
        let b = Position::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        assert_eq!(a.occupied, b.occupied);
        assert_eq!(a.castling_rights, b.castling_rights);
        assert_eq!(a.mailbox.0, b.mailbox.0);
    }

    #[test]
    fn test_missing_piece_placement() {
        // An entirely empty FEN string should trigger the first ok_or
        let res = Position::from_fen("");
        assert_eq!(res.unwrap_err(), "missing piece placement");
    }

    #[test]
    fn test_bad_rank_layout() {
        // Here, a '/' occurs after only 7 squares on a rank (e.g. ppppppp is 7 instead of 8)
        let res = Position::from_fen("ppppppp/8/8/8/8/8/8/8 w - - 0 1");
        assert_eq!(res.unwrap_err(), "bad rank layout");
    }

    #[test]
    fn test_bad_piece() {
        // 'X' is not a valid chess piece character
        let res = Position::from_fen("rnbqkbnX/8/8/8/8/8/8/8 w - - 0 1");
        assert!(res.unwrap_err().contains("bad piece 'X'"));
    }

    #[test]
    fn test_too_many_squares_in_rank() {
        // 5 + 4 = 9 squares, exceeding the rank width before hitting a '/'
        let res = Position::from_fen("5p4/8/8/8/8/8/8/8 w - - 0 1");
        assert_eq!(res.unwrap_err(), "too many squares in rank");
    }

    #[test]
    fn test_incomplete_piece_placement() {
        // Missing a full rank entirely (only 7 ranks out of 8 provided)
        let res = Position::from_fen("8/8/8/8/8/8/8 w - - 0 1");
        assert_eq!(res.unwrap_err(), "incomplete piece placement");
    }

    // --- 2. SIDE TO MOVE TESTS ---

    #[test]
    fn test_bad_side_to_move() {
        // Uses 'x' instead of 'w' or 'b'
        let res = Position::from_fen("8/8/8/8/8/8/8/8 x - - 0 1");
        assert_eq!(res.unwrap_err(), "bad side to move");
    }

    // --- 3. CASTLING RIGHTS TESTS ---

    #[test]
    fn test_missing_castling_field() {
        // Stops abruptly after side to move
        let res = Position::from_fen("8/8/8/8/8/8/8/8 w");
        assert_eq!(res.unwrap_err(), "missing castling field");
    }

    #[test]
    fn test_bad_castling_char() {
        // 'X' is an invalid castling character
        let res = Position::from_fen("8/8/8/8/8/8/8/8 w KX - 0 1");
        assert!(res.unwrap_err().contains("bad castling char 'X'"));
    }

    // --- 4. EN PASSANT SQUARE TESTS ---

    #[test]
    fn test_missing_en_passant_field() {
        // Stops abruptly after castling rights
        let res = Position::from_fen("8/8/8/8/8/8/8/8 w -");
        assert_eq!(res.unwrap_err(), "missing en passant field");
    }

    #[test]
    fn test_bad_en_passant_square_length() {
        // En passant square should be 2 characters (e.g. "e3"), "e33" is too long
        let res = Position::from_fen("8/8/8/8/8/8/8/8 w - e33 0 1");
        assert_eq!(res.unwrap_err(), "bad en passant square");
    }

    #[test]
    fn test_bad_en_passant_square_out_of_bounds() {
        // 'i' is out of bounds (files 'a' through 'h' only)
        let res = Position::from_fen("8/8/8/8/8/8/8/8 w - i3 0 1");
        assert_eq!(res.unwrap_err(), "bad en passant square");
    }

    // --- 5 & 6. CLOCK PARSING TESTS ---

    #[test]
    fn test_bad_halfmove_clock() {
        // "abc" cannot be parsed into an integer
        let res = Position::from_fen("8/8/8/8/8/8/8/8 w - - abc 1");
        assert_eq!(res.unwrap_err(), "bad halfmove clock");
    }

    #[test]
    fn test_bad_fullmove_number() {
        // "xyz" cannot be parsed into an integer
        let res = Position::from_fen("8/8/8/8/8/8/8/8 w - - 0 xyz");
        assert_eq!(res.unwrap_err(), "bad fullmove number");
    }
} // mod tests