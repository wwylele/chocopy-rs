mod check;
mod gen;
mod location;
mod node;
mod parse;

use location::*;
use node::*;
use std::fs::File;
use std::io::{BufRead, BufReader};

fn main() {
    let file = std::env::args().nth(1).unwrap();
    let ast = parse::process(&file).unwrap();

    let Ast::Program(Program {
        errors: ErrorInfo::Errors(Errors { errors, .. }),
        ..
    }) = &ast;
    if !errors.is_empty() {
        let mut errors: Vec<_> = errors
            .iter()
            .map(|e| {
                let Error::CompilerError(c) = e;
                c
            })
            .collect();
        errors.sort_by_key(|e| {
            let CompilerError {
                base:
                    NodeBase {
                        location: Location { start, .. },
                    },
                ..
            } = e;
            (start.row, start.col)
        });
        let file = File::open(file).unwrap();
        let mut lines = BufReader::new(file)
            .lines()
            .take_while(|l| l.is_ok())
            .map(|l| l.unwrap());
        let mut current_row = 1;
        let mut line = lines.next();
        for error in errors {
            let Location { start, end } = error.base.location;
            let row = start.row;
            if row > current_row {
                for _ in 0..row - current_row - 1 {
                    lines.next();
                }
                line = lines.next().map(|s| s.replace('\t', " "));
                current_row = row;
            }
            println!("{}, {}: {}", start.row, start.col, error.message);
            if let Some(line) = &line {
                println!("    | {}", line);
                print!("    | ");
                for _ in 0..std::cmp::max(start.col as i64 - 1, 0) {
                    print!(" ");
                }

                for _ in 0..std::cmp::max(end.col as i64 - start.col as i64 + 1, 1) {
                    print!("^");
                }

                println!();
            }
        }
    } else {
        println!("{}", serde_json::to_string_pretty(&ast).unwrap());
    }
}
