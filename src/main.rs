#![allow(dead_code)] // silences "unused" warnings while the engine is mostly empty

mod attacks;
mod bitboard;
mod movegen;
mod moves;
mod perft;
mod position;
mod sliders;
mod types;
mod uci;

use perft::{divide, perft};
use position::Position;

fn bench() {
    let pos = Position::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1").unwrap();
    for depth in 1..=5 {
        let t = std::time::Instant::now();
        let nodes = perft(&pos, depth);
        let secs = t.elapsed().as_secs_f64();
        println!("depth {depth}: {nodes} nodes, {secs:.3}s, {:.1} Mnps", nodes as f64 / secs / 1e6);
    }
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