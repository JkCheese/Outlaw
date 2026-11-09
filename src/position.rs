use std::fmt;
use crate::attacks::*;
use crate::bitboard::Bitboard;
use crate::moves::Move;
use crate::sliders::{bishop_attacks, rook_attacks};
use crate::types::*;
use crate::zobrist::{castling_key, ep_key, piece_key, KEYS};

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
    
    pub castling_rights: u8,
    pub castling_rook_sqs: [Square; 4],

    pub hash: u64,
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
            castling_rights: 0,
            castling_rook_sqs: [Square(0); 4],
            hash: castling_key(0), // matches compute_hash() for an empty board, White to move
        }
    }

    pub fn place_piece(&mut self, color: Color, piece: Piece, sq: Square) {
        debug_assert!(self.mailbox[sq].is_none(), "square already occupied");
        let bb = Bitboard::from_square(sq);
        self.pieces[color][piece] |= bb;
        self.occupied |= bb;
        self.occupied_by[color] |= bb;
        self.mailbox[sq] = Some((color, piece));
        self.hash ^= piece_key(color, piece, sq);
    }

    pub fn remove_piece(&mut self, sq: Square) {
        if let Some((color, piece)) = self.mailbox[sq].take() {
            let bb = !Bitboard::from_square(sq);
            self.pieces[color][piece] &= bb;
            self.occupied &= bb;
            self.occupied_by[color] &= bb;
            self.hash ^= piece_key(color, piece, sq);
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
        let castling: &str = fields.next().ok_or("missing castling field")?;
        if castling != "-" {
            for c in castling.chars() {
                let color = if c.is_ascii_uppercase() { Color::White } else { Color::Black };
                let rank: u8 = if color == Color::White { 0 } else { 7 };
                let king_bb = pos.pieces[color][Piece::King];
                if king_bb.is_empty() {
                    return Err("castling right without a king".to_string());
                }
                let king_sq = king_bb.lsb();
                if king_sq.rank() != rank {
                    return Err("castling king not on back rank".to_string());
                }
                let rook_at = |f: u8| pos.mailbox[Square::new(f, rank)] == Some((color, Piece::Rook));

                let lower = c.to_ascii_lowercase();
                let kingside = match lower {
                    'k' => true,
                    'q' => false,
                    'a'..='h' => (lower as u8 - b'a') > king_sq.file(),
                    _ => return Err(format!("bad castling char '{}'", c)),
                };
                let rook_file = match lower {
                    'k' => (king_sq.file() + 1..8).rev().find(|&f| rook_at(f)),   // outermost on the right
                    'q' => (0..king_sq.file()).find(|&f| rook_at(f)),             // outermost on the left
                    _ => Some(lower as u8 - b'a').filter(|&f| rook_at(f)),
                }
                .ok_or(format!("no rook for castling char '{}'", c))?;

                let bit = match (color, kingside) {
                    (Color::White, true) => CASTLE_WK,
                    (Color::White, false) => CASTLE_WQ,
                    (Color::Black, true) => CASTLE_BK,
                    (Color::Black, false) => CASTLE_BQ,
                };
                pos.castling_rights |= bit;
                pos.castling_rook_sqs[bit.trailing_zeros() as usize] = Square::new(rook_file, rank);
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

        // an en passant square only counts if a pawn can actually capture on it
        if let Some(ep) = pos.ep_sq {
            if !pos.ep_capturable(ep) {
                pos.ep_sq = None;
            }
        }

        // 5 and 6. Clocks (some FENs omit them, so default to 0 and 1)
        pos.half_move_clock = fields.next().unwrap_or("0").parse().map_err(|_| "bad halfmove clock")?;
        pos.full_move_count = fields.next().unwrap_or("1").parse().map_err(|_| "bad fullmove number")?;

        pos.hash = pos.compute_hash();
        Ok(pos)
    }

    // can a pawn of the side to move capture en passant on `ep`?
    fn ep_capturable(&self, ep: Square) -> bool {
        let us = self.color_to_move;
        !(pawn_attacks(ep, us.flip()) & self.pieces[us][Piece::Pawn]).is_empty()
    }

    /// Full recomputation of the Zobrist hash. Slow: used by from_fen and the tests,
    /// while make_move keeps `hash` up to date incrementally.
    pub fn compute_hash(&self) -> u64 {
        let mut h = 0;
        for sq in self.occupied {
            if let Some((c, p)) = self.mailbox[sq] {
                h ^= piece_key(c, p, sq);
            }
        }
        if self.color_to_move == Color::Black {
            h ^= KEYS.side;
        }
        h ^= castling_key(self.castling_rights);
        if let Some(ep) = self.ep_sq {
            h ^= ep_key(ep);
        }
        h
    }

    /// The position after the side to move "passes". Used by null-move pruning, never in a real game.
    /// The en passant right disappears (and leaves the hash), and the clock restarts so that
    /// repetition look-back can't reach across the pass.
    pub fn make_null_move(&self) -> Position {
        let mut pos = *self;
        if let Some(ep) = self.ep_sq {
            pos.hash ^= ep_key(ep);
            pos.ep_sq = None;
        }
        pos.color_to_move = self.color_to_move.flip();
        pos.hash ^= KEYS.side;
        pos.half_move_clock = 0;
        pos
    }

    /// True when neither side can ever deliver mate: bare kings, or kings plus ONE minor piece.
    /// (Cheap early-out: with more than 3 pieces on the board this is never true.)
    pub fn insufficient_material(&self) -> bool {
        if self.occupied.count() > 3 {
            return false;
        }
        [Color::White, Color::Black].iter().all(|&c| {
            (self.pieces[c][Piece::Pawn] | self.pieces[c][Piece::Rook] | self.pieces[c][Piece::Queen]).is_empty()
        })
    }

    pub fn is_capture(&self, m: Move) -> bool {
        match m.kind() {
            Move::EN_PASSANT => true,
            Move::CASTLING => false,
            _ => self.mailbox[m.to_sq()].is_some(),
        }
    }

    pub fn is_square_attacked(&self, sq: Square, by: Color) -> bool {
        self.is_square_attacked_occ(sq, by, self.occupied)
    }

    pub fn is_square_attacked_occ(&self, sq: Square, by: Color, occ: Bitboard) -> bool {
        let theirs = &self.pieces[by];
        !(pawn_attacks(sq, by.flip()) & theirs[Piece::Pawn]).is_empty()
            || !(knight_attacks(sq) & theirs[Piece::Knight]).is_empty()
            || !(king_attacks(sq) & theirs[Piece::King]).is_empty()
            || !(bishop_attacks(sq, occ) & (theirs[Piece::Bishop] | theirs[Piece::Queen])).is_empty()
            || !(rook_attacks(sq, occ) & (theirs[Piece::Rook] | theirs[Piece::Queen])).is_empty()
    }

    pub fn make_move(&self, m: Move) -> Position {
        let mut pos = *self;
        let us = self.color_to_move;
        let from = m.from_sq();
        let to = m.to_sq();
        let (_, piece) = self.mailbox[from].expect("no piece on from square");
        let was_capture = self.is_capture(m);

        // take the old castling rights and en passant square out of the hash
        pos.hash ^= castling_key(self.castling_rights);
        if let Some(ep) = self.ep_sq {
            pos.hash ^= ep_key(ep);
        }

        match m.kind() {
            Move::CASTLING => {
                let rook_from = to;
                let rank = from.rank();
                let (king_file, rook_file) = if rook_from.file() > from.file() { (6, 5) } else { (2, 3) };
                pos.remove_piece(from);
                pos.remove_piece(rook_from);
                pos.place_piece(us, Piece::King, Square::new(king_file, rank));
                pos.place_piece(us, Piece::Rook, Square::new(rook_file, rank));
            }
            Move::PROMOTION => {
                pos.remove_piece(to);
                pos.remove_piece(from);
                pos.place_piece(us, m.promotion_piece().unwrap(), to);
            }
            Move::EN_PASSANT => {
                pos.remove_piece(from);
                pos.remove_piece(Square::new(to.file(), from.rank())); // the captured pawn
                pos.place_piece(us, Piece::Pawn, to);
            }
            _ => {
                pos.remove_piece(to);
                pos.remove_piece(from);
                pos.place_piece(us, piece, to);
            }
        }

        // hand the move to the opponent first: ep_capturable looks at the side to move
        pos.color_to_move = us.flip();
        pos.hash ^= KEYS.side;

        pos.ep_sq = None;
        if piece == Piece::Pawn && (to.0 as i8 - from.0 as i8).abs() == 16 {
            let mid = Square((from.0 + to.0) / 2);
            if pos.ep_capturable(mid) {
                pos.ep_sq = Some(mid);
            }
        }

        // castling rights: a king move drops both of that side's rights,
        // and touching a rook's home square drops that rook's right
        if piece == Piece::King {
            pos.castling_rights &= !(if us == Color::White { CASTLE_WK | CASTLE_WQ } else { CASTLE_BK | CASTLE_BQ });
        }
        for i in 0..4 {
            let bit = 1u8 << i;
            if pos.castling_rights & bit != 0 && (from == pos.castling_rook_sqs[i] || to == pos.castling_rook_sqs[i]) {
                pos.castling_rights &= !bit;
            }
        }

        // put the new castling rights and en passant square into the hash
        pos.hash ^= castling_key(pos.castling_rights);
        if let Some(ep) = pos.ep_sq {
            pos.hash ^= ep_key(ep);
        }

        pos.half_move_clock = if piece == Piece::Pawn || was_capture { 0 } else { pos.half_move_clock.saturating_add(1) };
        if us == Color::Black { pos.full_move_count += 1; }
        pos
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
        let res = Position::from_fen("4k3/8/8/8/8/8/8/4K2R w KX - 0 1");
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

    #[test]
    fn insufficient_material_cases() {
        let yes = ["4k3/8/8/8/8/8/8/4K3 w - - 0 1", "4k3/8/8/8/8/8/8/3NK3 w - - 0 1", "4k3/8/8/8/8/8/8/3BK3 b - - 0 1", "3bk3/8/8/8/8/8/8/4K3 w - - 0 1"];
        let no = [
            "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1",   // a pawn can promote
            "4k3/8/8/8/8/8/8/3RK3 w - - 0 1",    // rook
            "4k3/8/8/8/8/8/8/3QK3 w - - 0 1",    // queen
            "4k3/8/8/8/8/8/8/2NNK3 w - - 0 1",   // two knights: not an automatic draw
            "3bk3/8/8/8/8/8/8/3BK3 w - - 0 1",   // two bishops: mate is possible on opposite colours
        ];
        for fen in yes { assert!(Position::from_fen(fen).unwrap().insufficient_material(), "{fen}"); }
        for fen in no { assert!(!Position::from_fen(fen).unwrap().insufficient_material(), "{fen}"); }
    }

    #[test]
    fn null_move_flips_side_and_keeps_hash_consistent() {
        for fen in [
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1", // en passant right must vanish
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R b KQkq - 3 7",
        ] {
            let pos = Position::from_fen(fen).unwrap();
            let n = pos.make_null_move();
            assert_eq!(n.color_to_move, pos.color_to_move.flip());
            assert_eq!(n.ep_sq, None);
            assert_eq!(n.hash, n.compute_hash(), "{fen}");
            assert_ne!(n.hash, pos.hash);
            assert_eq!(n.make_null_move().hash, Position { ep_sq: None, ..pos }.compute_hash()); // pass twice = same board
        }
    }

    #[test]
    fn hand_built_position_has_consistent_hash() {
        let mut pos = Position::new();
        pos.place_piece(Color::White, Piece::King, Square::new(4, 0));
        pos.place_piece(Color::Black, Piece::King, Square::new(4, 7));
        assert_eq!(pos.hash, pos.compute_hash());
    }

    // --- Zobrist hashing ---

    fn play(mut pos: Position, moves: &[&str]) -> Position {
        use crate::movegen::legal_moves;
        for text in moves {
            let m = legal_moves(&pos).as_slice().iter().copied()
                .find(|m| m.to_string() == *text)
                .unwrap_or_else(|| panic!("no legal move {text}"));
            pos = pos.make_move(m);
        }
        pos
    }

    #[test]
    fn known_hashes() {
        // white Ke1, Pe2; black Ke8; white to move; no castling or en passant
        let small = Position::from_fen("4k3/8/8/8/8/8/4P3/4K3 w - - 0 1").unwrap();
        assert_eq!(small.hash, 0xdc31_12ac_02b6_2999);
        assert_eq!(Position::startpos().hash, 0x9eaa_a48c_2fcc_dc6f);
    }

    #[test]
    fn transposed_move_orders_hash_equal() {
        let start = Position::startpos();
        let a = play(start, &["g1f3", "g8f6", "b1c3"]);
        let b = play(start, &["b1c3", "g8f6", "g1f3"]);
        assert_eq!(a.hash, b.hash);
        assert_eq!(a.hash, a.compute_hash());
    }

    #[test]
    fn hash_differs_by_side_and_rights() {
        let a = Position::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
        let b = Position::from_fen("r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1").unwrap();
        let c = Position::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w Qkq - 0 1").unwrap();
        assert_ne!(a.hash, b.hash);
        assert_ne!(a.hash, c.hash);
    }

    #[test]
    fn en_passant_square_only_kept_when_capturable() {
        // after 1.e4 nobody can capture on e3, so no en passant square and no ep key in the hash
        let pos = play(Position::startpos(), &["e2e4"]);
        assert_eq!(pos.ep_sq, None);
        assert_eq!(pos.hash, pos.compute_hash());
        // here a black pawn on d4 can capture on e3
        let pos = play(
            Position::from_fen("4k3/8/8/8/3p4/8/4P3/4K3 w - - 0 1").unwrap(),
            &["e2e4"],
        );
        assert_eq!(pos.ep_sq, Some(Square::new(4, 2)));
        assert_eq!(pos.hash, pos.compute_hash());
        // a FEN en passant square nobody can use is dropped
        let fen = Position::from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1").unwrap();
        assert_eq!(fen.ep_sq, None);
        assert_eq!(fen.hash, play(Position::startpos(), &["e2e4"]).hash);
    }
} // mod tests