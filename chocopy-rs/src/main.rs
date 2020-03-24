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
    let errors = &ast.program().errors.errors().errors;
    if errors.is_empty() {
        true
    } else {
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
                        ..
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

    let ast = parse::process(input)?;

    if matches.opt_present("ast") {
        println!("{}", serde_json::to_string_pretty(&ast).unwrap());
        return Ok(());
    }

    if !check_error(input, &ast) {
        return Ok(());
    }

    let ast = check::check(ast);

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

#[cfg(test)]
mod test {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    #[test]
    fn test_whole() {
        let temp_path = std::env::temp_dir();
        let mut passed = true;
        let test_dirs = ["test/pa3"];
        for dir in &test_dirs {
            println!("Testing Directory {}", dir);
            let mut files = std::fs::read_dir(dir)
                .unwrap()
                .map(|f| f.unwrap())
                .filter(|f| f.file_name().to_str().unwrap().ends_with(".py"))
                .map(|f| f.path())
                .collect::<Vec<_>>();
            files.sort();
            for file in files {
                let file_name = file.file_name().unwrap().to_owned();
                println!("Testing {}", file_name.to_str().unwrap());
                let exe_file = format!("chocopy-{}", rand::random::<u32>());
                let mut exe_path = temp_path.clone();
                exe_path.push(exe_file);

                let file = file.to_str().unwrap();
                let ast = parse::process(file).unwrap();
                assert!(check_error(file, &ast));
                let ast = check::check(ast);
                assert!(check_error(file, &ast));
                gen::gen(file, ast, exe_path.to_str().unwrap(), false).unwrap();

                let mut file = BufReader::new(std::fs::File::open(&file).unwrap());

                let mut case = 0;
                loop {
                    if !loop {
                        let mut line = "".to_owned();
                        if file.read_line(&mut line).unwrap() == 0 {
                            break false;
                        }
                        if line == "#!\n" {
                            break true;
                        }
                    } {
                        break;
                    }

                    print!("Case {} ---- ", case);

                    let process = std::process::Command::new(&exe_path)
                        .stdin(std::process::Stdio::piped())
                        .stdout(std::process::Stdio::piped())
                        .spawn()
                        .unwrap();

                    let mut stdin = process.stdin.unwrap();
                    let mut stdout = process.stdout.unwrap();

                    loop {
                        let mut line = "".to_owned();
                        file.read_line(&mut line).unwrap();
                        if line == "#<->#\n" {
                            break;
                        }
                        let bytes = line.as_bytes();
                        assert!(bytes[0] == b'#');

                        #[allow(unused_must_use)]
                        {
                            stdin.write(&bytes[1..bytes.len()]);
                        }
                    }

                    let mut expected_output: Vec<u8> = vec![];
                    loop {
                        let mut line = "".to_owned();
                        file.read_line(&mut line).unwrap();
                        if line == "#<->#\n" {
                            break;
                        }
                        let bytes = line.as_bytes();
                        assert!(bytes[0] == b'#');
                        expected_output.extend(bytes.iter().skip(1));
                    }

                    let mut actual_output = vec![];
                    stdout.read_to_end(&mut actual_output).unwrap();
                    if expected_output == actual_output {
                        println!("\x1b[32mOK\x1b[0m");
                    } else {
                        println!("\x1b[31mError\x1b[0m");
                        passed = false;
                    }

                    case += 1
                }

                std::fs::remove_file(exe_path).unwrap();
            }
        }
        assert_eq!(passed, true);
    }
}
