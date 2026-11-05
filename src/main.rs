#![allow(dead_code)] // silences "unused" warnings while the engine is mostly empty

mod attacks;
mod bitboard;
mod eval;
mod movegen;
mod moves;
mod perft;
mod position;
mod search;
mod sliders;
mod types;
mod uci;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use perft::{divide};
use position::Position;
use search::{search_nodes, Limits};

fn bench() {
    const FENS: [&str; 6] = [
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
    ];
    let t = std::time::Instant::now();
    let mut nodes = 0;
    for fen in FENS {
        let pos = Position::from_fen(fen).unwrap();
        let limits = Limits { depth: Some(8), ..Default::default() };
        nodes += search_nodes(&pos, &limits, Arc::new(AtomicBool::new(false))).1;
    }
    let secs = t.elapsed().as_secs_f64();
    println!("{nodes} nodes {secs:.2}s {:.0} nps", nodes as f64 / secs);
}

fn main() {
    sliders::init();
    let args: Vec<String> = std::env::args().collect();
    match args.len() {
        1 => uci::run(),
        2 if args[1] == "bench" => bench(),
        3 => {
            let pos = Position::from_fen(&args[1]).expect("bad FEN");
            divide(&pos, args[2].parse().expect("bad depth"));
        }
        _ => eprintln!("usage: outlaw | outlaw bench | outlaw \"<fen>\" <depth>"),
    }
}