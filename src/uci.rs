use std::time;

use crate::{
    evaluate::defs::DEFAULT_HASH_SIZE,
    search::{
        defs::{SearchLimits, FEN_START_POSITION},
        Search,
    },
};

use crate::benchmark::FENS;

pub struct UCI {}

impl UCI {
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
                println!("uciok");
            } else if token == "xboard" {
                println!("This engine does not support the xboard protocol.");
                token = "quit";
            } else if token == "isready" {
                println!("readyok");
            } else if token == "ucinewgame" {
                search.position.set(FEN_START_POSITION.to_string());
            } else if token == "position" {
                UCI::position(search, &mut args);
            } else if token == "go" {
                UCI::go(search, &mut args);
            } else if token == "setoption" {
                UCI::option(search, &mut args);
            } else if token == "bench" {
                UCI::bench(search);
            } else if token == "help" {
                UCI::help();
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
            token = args.next().unwrap_or("");
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
                    limits.white_inc = args.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
                }
                "binc" => {
                    limits.black_inc = args.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
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

                    if selected_option == "Hash" { search.eval.resize_transposition_table(
                        value.parse::<usize>().unwrap_or(DEFAULT_HASH_SIZE).min(512).max(1),
                    ) }
                }
                _ => (),
            }

            token = args.next().unwrap_or("");
        }
    }

    fn bench(search: &mut Search) {
        let mut nodes: usize = 0;
        let mut count: usize = 1;
        let elapsed = time::Instant::now();

        for fen in FENS {
            println!("\nPosition: {}/{}, ({})", count, FENS.len(), fen);
            count += 1;
            search.position.set(fen.to_string());
            search.run(SearchLimits::default());
            nodes += search.nodes_searched;
        }

        let duration = time::Instant::now() - elapsed + time::Duration::from_millis(1); // Ensure positivity to avoid a 'divide by zero'

        println!("\n===========================");
        println!("Total time (ms) : {}", duration.as_millis());
        println!("Nodes searched  : {}", nodes);
        println!("Nodes/second    : {}", 1000 * nodes / duration.as_millis() as usize);
    }

    fn help() {
        println!();
        println!("Oxide is a simple chess engine I built as a learning project.");
        println!("It is UCI compatible and can be used with any UCI compatible GUI.");
        println!("While not very strong yet but I am working on it and hoping to achieve a rating of 2000+.");
        println!();
    }
}
