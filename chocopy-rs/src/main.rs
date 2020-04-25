mod check;
mod gen;
mod local_env;
mod location;
mod node;
mod parse;

use gen::Platform;
use getopts::Options;
use location::*;
use node::*;
use std::fs::File;
use std::io::{BufRead, BufReader};

#[cfg(target_os = "windows")]
const PLATFORM: Platform = Platform::Windows;

#[cfg(target_os = "linux")]
const PLATFORM: Platform = Platform::Linux;

#[cfg(target_os = "macos")]
const PLATFORM: Platform = Platform::Macos;

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} INPUT [OUTPUT] [OPTIONS]", program);
    print!("{}", opts.usage(&brief));
}

fn check_error(file: &str, ast: &Program) -> bool {
    let errors = &ast.errors.errors;
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
            eprintln!("{}, {}: {}", start.row, start.col, error.message);
            if let Some(line) = &line {
                eprintln!("    | {}", line);
                eprint!("    | ");
                for _ in 0..std::cmp::max(start.col as i64 - 1, 0) {
                    eprint!(" ");
                }
                eprintln!("^");
            }
        }
        false
    }
}

#[derive(Debug)]
struct ArgumentError;

impl std::fmt::Display for ArgumentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid argument")
    }
}

impl std::error::Error for ArgumentError {}

#[derive(Debug)]
struct CodeError;

impl std::fmt::Display for CodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid source code")
    }
}

impl std::error::Error for CodeError {}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optflag("h", "help", "print this help menu");
    opts.optflag("a", "ast", "print bare AST");
    opts.optflag("t", "typed", "print typed AST");
    opts.optflag("o", "obj", "output object file without linking");
    opts.optflag("s", "static", "Link against library statically if possible");
    opts.optopt(
        "p",
        "platform",
        "Specify target platform",
        "[windows|linux|macos]",
    );

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            eprintln!("Failed to parse the arguments: {}", f);
            print_usage(&program, opts);
            return Err(ArgumentError.into());
        }
    };

    if matches.opt_present("h") {
        print_usage(&program, opts);
        return Ok(());
    }

    let input = if let Some(input) = matches.free.get(0) {
        input
    } else {
        eprintln!("Please specifiy source file");
        return Err(ArgumentError.into());
    };

    let ast = parse::process(input)?;

    if matches.opt_present("ast") {
        println!("{}", serde_json::to_string_pretty(&ast).unwrap());
        return Ok(());
    }

    if !check_error(input, &ast) {
        return Err(CodeError.into());
    }

    let ast = check::check(ast);

    if matches.opt_present("typed") {
        println!("{}", serde_json::to_string_pretty(&ast).unwrap());
        return Ok(());
    }

    if !check_error(input, &ast) {
        return Err(CodeError.into());
    }

    let output = if let Some(output) = matches.free.get(1) {
        output
    } else {
        eprintln!("Please specifiy output path");
        return Err(ArgumentError.into());
    };

    let no_link = matches.opt_present("o");
    let static_lib = matches.opt_present("s");
    let platform = matches
        .opt_str("platform")
        .map(|p| match p.as_str() {
            "windows" => Ok(Platform::Windows),
            "linux" => Ok(Platform::Linux),
            "macos" => Ok(Platform::Macos),
            _ => {
                eprintln!("Unknown platform `{}`", p);
                Err(ArgumentError)
            }
        })
        .transpose()?
        .unwrap_or(PLATFORM);

    if platform != PLATFORM && !no_link {
        eprintln!("Cross-platform linking is unsupported. Please use --obj option.");
        return Err(ArgumentError.into());
    }

    gen::gen(input, ast, output, no_link, static_lib, platform)?;

    Ok(())
}
