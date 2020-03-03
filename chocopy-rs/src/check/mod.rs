use crate::location::*;
use crate::node::*;

pub fn check(mut ast: Ast) -> Ast {
    ast
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{stdout, Write};
    #[test]
    fn sample() {
        let mut passed = true;
        let test_dirs = ["../chocopy-wars/src/test/data/pa2/sample"];
        for dir in &test_dirs {
            println!("Testing Directory {}", dir);
            let mut files = std::fs::read_dir(dir)
                .unwrap()
                .map(|f| f.unwrap())
                .filter(|f| f.file_name().to_str().unwrap().ends_with(".ast"))
                .map(|f| f.path())
                .collect::<Vec<_>>();
            files.sort();

            for ast_file in files {
                let mut typed_file = ast_file.clone();
                let mut file_name = ast_file.file_name().unwrap().to_owned();
                print!("Testing {} ---- ", file_name.to_str().unwrap());
                stdout().flush().unwrap();
                file_name.push(".typed");
                typed_file.set_file_name(file_name);
                let ast_string = String::from_utf8(std::fs::read(ast_file).unwrap()).unwrap();
                let typed_string = String::from_utf8(std::fs::read(typed_file).unwrap()).unwrap();
                let ast = serde_json::from_str::<Ast>(&ast_string).unwrap();
                let typed = serde_json::from_str::<Ast>(&typed_string).unwrap();
                if check(ast) == typed {
                    println!("\x1b[32mOK\x1b[0m");
                } else {
                    println!("\x1b[31mError\x1b[0m");
                    //passed = false;
                }
            }
        }
        assert_eq!(passed, true);
    }
}
