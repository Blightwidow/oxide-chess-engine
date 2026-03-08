use std::time;

use crate::search::{
    defs::{SearchLimits, FEN_START_POSITION},
    Search,
};

use crate::benchmark::FENS;

pub struct Uci {}

impl Uci {
    pub fn main_loop(search: &mut Search) {
        // Handle stream

        let argc = std::env::args().len();
        let mut buffer: String = std::env::args().skip(1).collect::<Vec<String>>().join(" ");

        loop {
            if argc == 1 {
                let read_result = std::io::stdin().read_line(&mut buffer);

                if read_result.is_err() {
                    buffer = "quit".to_string();
                }
            }

            let cmd: String = buffer.clone();
            let mut args: std::str::SplitWhitespace<'_> = cmd.split_whitespace();
            let mut token = args.next().unwrap_or("");
            buffer.clear();

            if token == "uci" {
                println!("id name Oxide");
                println!("id author Theo Dammaretz");
                println!("option name Hash type spin default 128 min 1 max 512");
                println!("option name EvalFile type string default nets/default.nnue");
                println!("uciok");
            } else if token == "xboard" {
                println!("This engine does not support the xboard protocol.");
                token = "quit";
            } else if token == "isready" {
                println!("readyok");
            } else if token == "ucinewgame" {
                search.position.set(FEN_START_POSITION.to_string());
                search.eval.transposition_table.clear();
            } else if token == "position" {
                Uci::position(search, &mut args);
            } else if token == "go" {
                // TODO: Update once multithreading is implemented
                println!("info string Using 1 thread");
                Uci::go(search, &mut args);
            } else if token == "setoption" {
                Uci::option(search, &mut args);
            } else if token == "bench" {
                Uci::bench(search, &mut args);
            } else if token == "bench_perft" {
                Uci::bench_perft(search, &mut args);
            } else if token == "help" {
                Uci::help();
            } else if !token.is_empty() && token.chars().nth(0).unwrap_or_default() != '#' {
                println!("Unknown command: {}. Type help for more information", token);
            }

            if token == "quit" || argc > 1 {
                break;
            }
        }
    }

    fn position(search: &mut Search, args: &mut std::str::SplitWhitespace<'_>) {
        let mut token = args.next().unwrap_or("");

        if token == "startpos" {
            search.position.set(FEN_START_POSITION.to_string());

            // Consume the next token if it is 'moves'
            args.next();
        } else if token == "fen" {
            let mut fen = String::new();

            while token != "moves" && !token.is_empty() {
                token = args.next().unwrap_or("");
                fen += token;
                fen += " ";
            }

            search.position.set(fen);
        }

        // Move to first move if any
        token = args.next().unwrap_or("");

        while !token.is_empty() {
            let mv_string = token.to_ascii_lowercase();

            for mv in search.movegen.legal_moves(&search.position) {
                if mv_string == format!("{:?}", mv) {
                    search.position.do_move(mv);
                    break;
                }
            }

            token = args.next().unwrap_or("");
        }
    }

    fn go(search: &mut Search, args: &mut std::str::SplitWhitespace<'_>) {
        let mut limits = SearchLimits::default();
        let mut token = args.next().unwrap_or("");

        while !token.is_empty() {
            match token {
                "perft" => {
                    limits.perft = args.next().unwrap_or("1").parse::<u8>().unwrap_or(1);
                }
                "depth" => {
                    limits.depth = args.next().unwrap_or("1").parse::<u8>().unwrap_or(1);
                }
                "ponder" => {
                    limits.ponder = true;
                }
                "wtime" => {
                    limits.white_time = args.next().unwrap_or("0").parse::<u64>().unwrap_or(0);
                }
                "btime" => {
                    limits.black_time = args.next().unwrap_or("0").parse::<u64>().unwrap_or(0);
                }
                "winc" => {
                    limits.white_inc = args.next().unwrap_or("0").parse::<u64>().unwrap_or(0u64);
                }
                "binc" => {
                    limits.black_inc = args.next().unwrap_or("0").parse::<u64>().unwrap_or(0u64);
                }
                "movestogo" => {
                    limits.moves_to_go = args.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
                }
                "nodes" => {
                    limits.nodes = args.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
                }
                "mate" => {
                    limits.mate = args.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
                }
                "movetime" => {
                    limits.movetime = args.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
                }
                "infinite" => {
                    limits.depth = u8::MAX;
                }
                _ => (),
            }

            token = args.next().unwrap_or("");
        }

        search.run(limits);
    }

    fn option(search: &mut Search, args: &mut std::str::SplitWhitespace<'_>) {
        let mut token = args.next().unwrap_or("");
        let mut selected_option = "";

        while !token.is_empty() {
            match token {
                "name" => {
                    selected_option = args.next().unwrap_or("");
                }
                "value" => {
                    let value = args.next().unwrap_or("");

                    if selected_option == "Hash" {
                        search
                            .eval
                            .resize_transposition_table(value.parse::<usize>().unwrap_or(16).clamp(1, 512));
                    } else if selected_option == "EvalFile" {
                        search.load_nnue(value);
                    }
                }
                _ => (),
            }

            token = args.next().unwrap_or("");
        }
    }

    fn bench(search: &mut Search, args: &mut std::str::SplitWhitespace<'_>) {
        let mut nodes: usize = 0;
        let elapsed = time::Instant::now();

        let mut limits = SearchLimits::default();

        limits.hash_size = args.next().unwrap_or("16").parse::<usize>().unwrap_or(16);
        limits.threads = args.next().unwrap_or("1").parse::<usize>().unwrap_or(1);
        limits.depth = args.next().unwrap_or("13").parse::<u8>().unwrap_or(13);

        for (i, fen) in FENS.iter().enumerate() {
            println!("\nPosition: {}/{} ({})", i + 1, FENS.len(), fen);
            search.eval.transposition_table.clear();
            search.position.set(fen.to_string());
            search.run(limits);
            nodes += search.nodes_searched;
        }

        let duration = time::Instant::now() - elapsed + time::Duration::from_millis(1); // Ensure positivity to avoid a 'divide by zero'

        println!("\n===========================");
        println!("Total time (ms) : {}", duration.as_millis());
        println!("Nodes searched  : {}", nodes);
        println!("Nodes/second    : {}", 1000 * nodes / duration.as_millis() as usize);
    }

    fn bench_perft(search: &mut Search, _args: &mut std::str::SplitWhitespace<'_>) {
        struct PerftSuite {
            name: &'static str,
            fen: &'static str,
            max_depth: u8,
        }

        let suites = [
            PerftSuite {
                name: "Start",
                fen: FEN_START_POSITION,
                max_depth: 7,
            },
            PerftSuite {
                name: "Kiwipete",
                fen: "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
                max_depth: 6,
            },
            PerftSuite {
                name: "Midgame",
                fen: "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
                max_depth: 6,
            },
            PerftSuite {
                name: "Endgame",
                fen: "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - -",
                max_depth: 6,
            },
        ];

        let mut total_nodes: u64 = 0;
        let total_start = time::Instant::now();

        for suite in &suites {
            search.position.set(suite.fen.to_string());

            for depth in 1..=suite.max_depth {
                let start = time::Instant::now();
                let nodes = search.perft(depth, false);
                let elapsed_ms = start.elapsed().as_millis();
                let mnps = if elapsed_ms > 0 {
                    nodes as f64 / elapsed_ms as f64 / 1000.0
                } else {
                    nodes as f64 / 1000.0
                };

                println!(
                    "Perft {} {}: {} {}ms {} MNodes/s",
                    suite.name, depth, nodes, elapsed_ms, mnps
                );

                total_nodes += nodes;
            }

            println!();
        }

        let total_ms = total_start.elapsed().as_millis();
        let total_mnps = if total_ms > 0 {
            total_nodes as f64 / total_ms as f64 / 1000.0
        } else {
            total_nodes as f64 / 1000.0
        };

        println!(
            "Perft aggregate: {} {}ms {} MNodes/s",
            total_nodes, total_ms, total_mnps
        );
    }

    fn help() {
        println!();
        println!("Oxide is a simple chess engine I built as a learning project.");
        println!("It is UCI compatible and can be used with any UCI compatible GUI.");
        println!("While not very strong yet but I am working on it and hoping to achieve a rating of 2000+.");
        println!();
    }
}
