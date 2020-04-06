mod check;
mod gen;
mod local_env;
mod location;
mod node;
mod parse;

use getopts::Options;
use location::*;
use node::*;
use std::fs::File;
use std::io::{BufRead, BufReader};

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} INPUT [-a|-t|[-n] OUTPUT]", program);
    print!("{}", opts.usage(&brief));
}

fn check_error(file: &str, ast: &Ast) -> bool {
    let errors = &ast.program().errors.errors;
    if errors.is_empty() {
        true
    } else {
        let file = File::open(file).unwrap();
        let mut lines = BufReader::new(file)
            .lines()
            .take_while(|l| l.is_ok())
            .map(|l| l.unwrap());
        let mut current_row = 1;
        let mut line = lines.next();
        for error in errors {
            let Location { start, .. } = error.base.location;
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
                println!("^");
            }
        }
        false
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optflag("h", "help", "print this help menu");
    opts.optflag("a", "ast", "output bare AST");
    opts.optflag("t", "typed", "output typed AST");
    opts.optflag("n", "no-link", "output object file without linking");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            println!("Failed to parse the arguments: {}", f);
            print_usage(&program, opts);
            return Ok(());
        }
    };

    if matches.opt_present("h") {
        print_usage(&program, opts);
        return Ok(());
    }

    let input = if let Some(input) = matches.free.get(0) {
        input
    } else {
        println!("Please specifiy source file");
        return Ok(());
    };

    let mut ast = parse::process(input)?;
    ast.program_mut().errors.sort();

    if matches.opt_present("ast") {
        println!("{}", serde_json::to_string_pretty(&ast).unwrap());
        return Ok(());
    }

    if !check_error(input, &ast) {
        return Ok(());
    }

    let mut ast = check::check(ast);
    ast.program_mut().errors.sort();

    if matches.opt_present("typed") {
        println!("{}", serde_json::to_string_pretty(&ast).unwrap());
        return Ok(());
    }

    if !check_error(input, &ast) {
        return Ok(());
    }

    let output = if let Some(output) = matches.free.get(1) {
        output
    } else {
        println!("Please specifiy output path");
        return Ok(());
    };

    gen::gen(input, ast, output, matches.opt_present("n"))?;

    Ok(())
}
