use std::io::{self, BufRead};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use crate::movegen::legal_moves;
use crate::moves::Move;
use crate::perft::divide;
use crate::position::Position;
use crate::search::{search, Limits};
use crate::tt::Tt;

const DEFAULT_HASH_MB: usize = 16;

// find the legal move whose text matches what the GUI sent
fn parse_move(pos: &Position, text: &str, chess960: bool) -> Option<Move> {
    legal_moves(pos).as_slice().iter().copied().find(|&m| m.to_uci(chess960) == text)
}

// returns the final position and the hash of every position before it (for repetition detection)
fn parse_position(args: &[&str], chess960: bool) -> Result<(Position, Vec<u64>), String> {
    let (mut pos, i) = match args.first() {
        Some(&"startpos") => (Position::startpos(), 1),
        Some(&"fen") => {
            let end = args.iter().position(|&t| t == "moves").unwrap_or(args.len());
            (Position::from_fen(&args[1..end].join(" "))?, end)
        }
        _ => return Err("expected startpos or fen".to_string()),
    };
    let mut history = Vec::new();
    if args.get(i) == Some(&"moves") {
        for tok in &args[i + 1..] {
            let m = parse_move(&pos, tok, chess960).ok_or(format!("illegal move {tok}"))?;
            history.push(pos.hash);
            pos = pos.make_move(m);
        }
    }
    Ok((pos, history))
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
            "nodes" => lim.nodes = value,
            "infinite" => lim.infinite = true,
            _ => {}
        }
    }
    lim
}

// The search thread owns the table while it runs and hands it back when it ends.
type Worker = JoinHandle<Tt>;

// Stop any running search and take the table back.
fn finish(worker: &mut Option<Worker>, tt: &mut Option<Tt>, stop: &AtomicBool) {
    if let Some(w) = worker.take() {
        stop.store(true, Ordering::Relaxed);
        *tt = Some(w.join().expect("search thread panicked"));
    }
}

pub fn run() {
    let mut pos = Position::startpos();
    let mut history: Vec<u64> = Vec::new();
    let mut chess960 = false;

    let mut tt = Some(Tt::new(DEFAULT_HASH_MB));
    let mut stop = Arc::new(AtomicBool::new(false));
    let mut worker: Option<Worker> = None;

    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let mut tokens = line.split_whitespace();
        let Some(cmd) = tokens.next() else { continue };
        let args: Vec<&str> = tokens.collect();

        match cmd {
            "uci" => {
                println!("id name outlaw {}", env!("CARGO_PKG_VERSION"));
                println!("id author <your name>");
                println!("option name Hash type spin default {DEFAULT_HASH_MB} min 1 max 4096");
                println!("option name UCI_Chess960 type check default false");
                println!("uciok");
            }
            "isready" => println!("readyok"),
            "ucinewgame" => {
                finish(&mut worker, &mut tt, &stop);
                if let Some(t) = tt.as_mut() { t.clear(); }
                pos = Position::startpos();
                history.clear();
            }
            "setoption" => {
                // setoption name <name> value <value>
                if args.len() >= 4 {
                    match args[1] {
                        "UCI_Chess960" => chess960 = args[3] == "true",
                        "Hash" => {
                            if let Ok(mb) = args[3].parse::<usize>() {
                                finish(&mut worker, &mut tt, &stop);
                                tt = Some(Tt::new(mb.clamp(1, 4096)));
                            }
                        }
                        _ => {}
                    }
                }
            }
            "position" => match parse_position(&args, chess960) {
                Ok((p, h)) => {
                    pos = p;
                    history = h;
                }
                Err(e) => println!("info string {e}"),
            },
            "go" => {
                finish(&mut worker, &mut tt, &stop); // end any previous search first
                if args.first() == Some(&"perft") {
                    let depth = args.get(1).and_then(|d| d.parse().ok()).unwrap_or(1);
                    divide(&pos, depth);
                } else {
                    stop = Arc::new(AtomicBool::new(false));
                    let mut limits = parse_go(&args);
                    limits.chess960 = chess960; // so `info ... pv` prints castling the way the GUI expects
                    let p = pos; // Position is Copy
                    let h = history.clone();
                    let flag = Arc::clone(&stop);
                    let mut table = tt.take().expect("table is not with the search thread");
                    worker = Some(std::thread::spawn(move || {
                        match search(&p, &limits, flag, h, &mut table) {
                            Some(m) => println!("bestmove {}", m.to_uci(chess960)),
                            None => println!("bestmove 0000"),
                        }
                        table
                    }));
                }
            }
            "stop" => stop.store(true, Ordering::Relaxed),
            "quit" => {
                finish(&mut worker, &mut tt, &stop);
                break;
            }
            _ => {} // unknown commands are ignored
        }
    }
}