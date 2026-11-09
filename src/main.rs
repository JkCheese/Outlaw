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
mod tt;
mod types;
mod uci;
mod zobrist;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use perft::{divide, perft};
use position::Position;
use search::{search_nodes, Limits};
use tt::Tt;

// Frozen list: never edit it, or old and new bench results stop being comparable.
const BENCH_FENS: [&str; 9] = [
    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
    "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
    "r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3",
    "8/8/4k3/8/8/4K3/4P3/8 w - - 0 1",
    "4r1k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1",
];

fn bench(depth: u32) {
    let t = std::time::Instant::now();
    let mut total = 0u64;
    let mut tt = Tt::new(16); // fixed size, cleared per position, so node counts are reproducible
    for (i, fen) in BENCH_FENS.iter().enumerate() {
        let pos = Position::from_fen(fen).unwrap();
        let limits = Limits { depth: Some(depth), quiet: true, ..Default::default() };
        tt.clear();
        let (_, nodes) = search_nodes(&pos, &limits, Arc::new(AtomicBool::new(false)), Vec::new(), &mut tt);
        println!("position {}: {} nodes", i + 1, nodes);
        total += nodes;
    }
    let secs = t.elapsed().as_secs_f64();
    println!("nodes {total} time {secs:.3} nps {:.0}", total as f64 / secs);
}

fn perft_bench() {
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
    match args.get(1).map(|s| s.as_str()) {
        None => uci::run(),
        Some("bench") => bench(args.get(2).and_then(|d| d.parse().ok()).unwrap_or(6)),
        Some("perftbench") => perft_bench(),
        Some(fen) if args.len() == 3 => {
            let pos = Position::from_fen(fen).expect("bad FEN");
            divide(&pos, args[2].parse().expect("bad depth"));
        }
        _ => eprintln!("usage: outlaw | bench [depth] | perftbench | \"<fen>\" <depth>"),
    }
}