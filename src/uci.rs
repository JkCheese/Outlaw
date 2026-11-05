use std::io::{self, BufRead};

use crate::movegen::legal_moves;
use crate::moves::Move;
use crate::perft::divide;
use crate::position::Position;
use crate::types::Square;

fn sq_name(sq: Square) -> String {
    format!("{}{}", (b'a' + sq.file()) as char, sq.rank() + 1)
}

// how a move is written on the wire; castling differs between standard and 960
fn uci_string(m: Move, chess960: bool) -> String {
    if m.kind() == Move::CASTLING && !chess960 {
        let from = m.from_sq();
        let file = if m.to_sq().file() > from.file() { 6 } else { 2 };
        format!("{}{}", sq_name(from), sq_name(Square::new(file, from.rank())))
    } else {
        m.to_string()
    }
}

// find the legal move whose text matches what the GUI sent
fn parse_move(pos: &Position, text: &str, chess960: bool) -> Option<Move> {
    legal_moves(pos).as_slice().iter().copied().find(|&m| uci_string(m, chess960) == text)
}

fn parse_position(args: &[&str], chess960: bool) -> Result<Position, String> {
    let (mut pos, i) = match args.first() {
        Some(&"startpos") => (Position::startpos(), 1),
        Some(&"fen") => {
            let end = args.iter().position(|&t| t == "moves").unwrap_or(args.len());
            (Position::from_fen(&args[1..end].join(" "))?, end)
        }
        _ => return Err("expected startpos or fen".to_string()),
    };
    if args.get(i) == Some(&"moves") {
        for tok in &args[i + 1..] {
            let m = parse_move(&pos, tok, chess960).ok_or(format!("illegal move {tok}"))?;
            pos = pos.make_move(m);
        }
    }
    Ok(pos)
}

#[derive(Default)]
struct Limits {
    depth: Option<u32>,
    movetime: Option<u64>,
    wtime: Option<u64>,
    btime: Option<u64>,
    winc: Option<u64>,
    binc: Option<u64>,
    infinite: bool,
}

fn parse_go(args: &[&str]) -> Limits {
    let mut lim = Limits::default();
    for i in 0..args.len() {
        let value = args.get(i + 1).and_then(|v| v.parse::<u64>().ok());
        match args[i] {
            "depth" => lim.depth = value.map(|v| v as u32),
            "movetime" => lim.movetime = value,
            "wtime" => lim.wtime = value,
            "btime" => lim.btime = value,
            "winc" => lim.winc = value,
            "binc" => lim.binc = value,
            "infinite" => lim.infinite = true,
            _ => {}
        }
    }
    lim
}

// placeholder until real search exists: play the first legal move
fn search(pos: &Position, _limits: &Limits) -> Option<Move> {
    legal_moves(pos).as_slice().first().copied()
}

pub fn run() {
    let mut pos = Position::startpos();
    let mut chess960 = false;

    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let mut tokens = line.split_whitespace();
        let Some(cmd) = tokens.next() else { continue };
        let args: Vec<&str> = tokens.collect();

        match cmd {
            "uci" => {
                println!("id name outlaw");
                println!("id author <your name>");
                println!("option name UCI_Chess960 type check default false");
                println!("uciok");
            }
            "isready" => println!("readyok"),
            "ucinewgame" => pos = Position::startpos(),
            "setoption" => {
                // setoption name <name> value <value>
                if args.len() >= 4 && args[1] == "UCI_Chess960" {
                    chess960 = args[3] == "true";
                }
            }
            "position" => match parse_position(&args, chess960) {
                Ok(p) => pos = p,
                Err(e) => println!("info string {e}"),
            },
            "go" => {
                if args.first() == Some(&"perft") {
                    let depth = args.get(1).and_then(|d| d.parse().ok()).unwrap_or(1);
                    divide(&pos, depth);
                } else {
                    match search(&pos, &parse_go(&args)) {
                        Some(m) => println!("bestmove {}", uci_string(m, chess960)),
                        None => println!("bestmove 0000"), // no legal moves
                    }
                }
            }
            "quit" => break,
            _ => {} // unknown commands are ignored
        }
    }
}