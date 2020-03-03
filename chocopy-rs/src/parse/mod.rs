mod lexer;
mod parser;
mod pipe;
mod token;
use crate::node::*;

pub fn process(path: &str) -> Result<Ast, Box<dyn std::error::Error>> {
    use async_std::fs::*;
    use async_std::io::*;
    use futures::executor::block_on;
    use futures::future::join;
    use std::cell::*;
    use std::rc::*;

    let file = Rc::new(RefCell::new(BufReader::new(block_on(File::open(path))?)));
    let get_char = move || {
        let file = file.clone();
        async move {
            let mut buf = [0];
            match file.borrow_mut().read_exact(&mut buf).await {
                Ok(()) if buf[0] < 0x80 => Some(buf[0] as char),
                _ => None,
            }
        }
    };

    let (put_token, get_token) = pipe::create_pipe();

    let ((), ast) = block_on(join(
        lexer::lex(get_char, put_token),
        parser::parse(get_token),
    ));

    Ok(ast)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{stdout, Write};

    fn compare_ast(a: &Ast, b: &Ast) -> bool {
        let Ast::Program(Program {
            errors: ErrorInfo::Errors(Errors {
                errors: a_errors, ..
            }),
            ..
        }) = a;
        let Ast::Program(Program {
            errors: ErrorInfo::Errors(Errors {
                errors: b_errors, ..
            }),
            ..
        }) = b;
        if a_errors.is_empty() {
            b_errors.is_empty() && a == b
        } else {
            if b_errors.is_empty() {
                return false;
            }
            let Error::CompilerError(CompilerError { base: a_base, .. }) = &a_errors[0];
            let Error::CompilerError(CompilerError { base: b_base, .. }) = &b_errors[0];
            a_base == b_base
        }
    }

    #[test]
    fn sample() {
        let mut passed = true;

        let mut files = std::fs::read_dir("../chocopy-wars/src/test/data/pa1/sample")
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
            let ast_reference = serde_json::from_str::<Ast>(&ast_string).unwrap();

            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                sender.send(process(source_file.as_os_str().to_str().unwrap()).unwrap())
            });

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
        assert_eq!(passed, true);
    }
}
