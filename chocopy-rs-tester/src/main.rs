use std::io::{BufRead, BufReader, Read, Write};

fn main() {
    let temp_path = std::env::temp_dir();

    let dir = std::env::args().nth(1).expect("Path required");

    let mut compiler_path = std::env::current_exe().unwrap();
    compiler_path.set_file_name("chocopy-rs");

    let mut passed = 0;
    let mut total = 0;

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

        assert!(std::process::Command::new(&compiler_path)
            .arg(&file)
            .arg(&exe_path)
            .spawn()
            .unwrap()
            .wait()
            .unwrap()
            .success());

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
                passed += 1;
            } else {
                println!("\x1b[31mError\x1b[0m");
            }
            total += 1;
            case += 1
        }

        std::fs::remove_file(exe_path).unwrap();
    }

    println!("Passed / Total: {} / {}", passed, total);
    assert_eq!(passed, total)
}
