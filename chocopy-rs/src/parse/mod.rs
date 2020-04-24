mod generator;
mod lexer;
mod parser;
mod token;
use crate::node::*;

pub fn process(path: &str) -> Result<Program, Box<dyn std::error::Error>> {
    use std::fs::*;
    use std::io::*;
    let mut file = BufReader::new(File::open(path)?);
    let get_char = move || {
        let mut buf = [0];
        match file.read_exact(&mut buf) {
            Ok(()) if buf[0] < 0x80 => Some(buf[0] as char),
            _ => None,
        }
    };

    let driver = |put_token| lexer::lex(get_char, put_token);
    let get_token = generator::generator(driver);
    let mut ast = parser::parse(get_token);

    ast.errors.sort();

    Ok(ast)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{stdout, Write};

    fn compare_ast(a: &Program, b: &Program) -> bool {
        let Program {
            errors: Errors {
                errors: a_errors, ..
            },
            ..
        } = a;
        let Program {
            errors: Errors {
                errors: b_errors, ..
            },
            ..
        } = b;
        if a_errors.is_empty() {
            b_errors.is_empty() && a == b
        } else {
            if b_errors.is_empty() {
                return false;
            }
            let CompilerError { base: a_base, .. } = &a_errors[0];
            let CompilerError { base: b_base, .. } = &b_errors[0];
            a_base == b_base
        }
    }

    #[test]
    fn sample() {
        let mut passed = true;

        let test_dirs = [
            "test/original/pa1",
            "test/original/pa1/hidden",
            "test/original/pa2",
            "test/pa1",
            "test/pa2",
        ];

        for dir in &test_dirs {
            println!("Testing Directory {}", dir);
            let mut files = std::fs::read_dir(dir)
                .unwrap()
                .map(|f| f.unwrap())
                .filter(|f| f.file_name().to_str().unwrap().ends_with(".py"))
                .map(|f| f.path())
                .collect::<Vec<_>>();

            files.sort();

            for source_file in files {
                let mut ast_file = source_file.clone();
                let mut file_name = ast_file.file_name().unwrap().to_owned();
                print!("Testing {} ---- ", file_name.to_str().unwrap());
                stdout().flush().unwrap();
                file_name.push(".ast");
                ast_file.set_file_name(file_name);

                let ast_string = String::from_utf8(std::fs::read(ast_file).unwrap()).unwrap();
                let ast_reference = serde_json::from_str::<Program>(&ast_string).unwrap();

                let (sender, receiver) = std::sync::mpsc::channel();
                std::thread::Builder::new()
                    .stack_size(16_000_000)
                    .spawn(move || {
                        sender.send(process(source_file.as_os_str().to_str().unwrap()).unwrap())
                    })
                    .unwrap();

                if let Ok(ast) = receiver.recv_timeout(std::time::Duration::from_secs(1)) {
                    if compare_ast(&ast, &ast_reference) {
                        println!("\x1b[32mOK\x1b[0m");
                    } else {
                        println!("\x1b[31mError\x1b[0m");
                        passed = false;
                    }
                } else {
                    println!("\x1b[31mTimeout\x1b[0m");
                    passed = false;
                }
            }
        }
        assert_eq!(passed, true);
    }
}
