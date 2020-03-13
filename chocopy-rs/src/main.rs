mod check;
mod gen;
mod location;
mod node;
mod parse;

use location::*;
use node::*;
use std::fs::File;
use std::io::{BufRead, BufReader};

fn compile(file: &str, out: Option<&str>) -> Option<Ast> {
    let mut ast = parse::process(&file).unwrap();
    if ast.program().errors.errors().errors.is_empty() {
        ast = check::check(ast);
    }

    if !ast.program().errors.errors().errors.is_empty() {
        return Some(ast);
    }

    if let Some(out) = out {
        gen::gen(ast, out).unwrap();
        None
    } else {
        Some(ast)
    }
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let file = &args[1];
    let out = args.get(2);

    let ast = compile(file, out.map(|s: &String| s.as_str()));

    if ast.is_none() {
        return;
    }

    let ast = ast.unwrap();

    let errors = &ast.program().errors.errors().errors;
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
    }

    println!("{}", serde_json::to_string_pretty(&ast).unwrap());
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    #[test]
    fn test_whole() {
        let mut temp_path = std::env::temp_dir();
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

                compile(file.to_str().unwrap(), Some(exe_path.to_str().unwrap()));

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
                        //passed = false;
                    }

                    case += 1
                }

                std::fs::remove_file(exe_path).unwrap();
            }
        }
    }
}
