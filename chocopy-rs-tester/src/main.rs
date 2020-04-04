use std::io::{BufRead, BufReader, Read, Write};

fn fixup_newline(s: &mut String) {
    if s.ends_with("\r\n") {
        s.pop();
        s.pop();
        s.push('\n');
    }
}

fn main() {
    let temp_path = std::env::temp_dir();

    let args: Vec<_> = std::env::args().collect();
    let dir = args.get(1).expect("Path required");
    let python = args.get(2).map(|s| s.as_str()) == Some("--python");
    let python_command;
    if python {
        python_command = Some(args.get(3).map(|s| s.as_str()).unwrap_or("python"));
        println!(
            "Testing using python interpreter {}",
            python_command.unwrap()
        );

        assert!(std::process::Command::new(python_command.unwrap())
            .arg("--version")
            .spawn()
            .unwrap()
            .wait()
            .unwrap()
            .success());
    } else {
        python_command = None;
        println!("Testing using chocopy compiler");
    }

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
    for file_path in files {
        let file_name = file_path.file_name().unwrap().to_owned();
        println!("Testing {}", file_name.to_str().unwrap());
        let exe_file = format!("chocopy-{}", rand::random::<u32>());
        let mut exe_path = temp_path.clone();
        exe_path.push(exe_file);

        if !python {
            assert!(std::process::Command::new(&compiler_path)
                .arg(&file_path)
                .arg(&exe_path)
                .spawn()
                .unwrap()
                .wait()
                .unwrap()
                .success());
        }

        let mut file = BufReader::new(std::fs::File::open(&file_path).unwrap());

        let mut case = 0;
        loop {
            if !loop {
                let mut line = "".to_owned();
                if file.read_line(&mut line).unwrap() == 0 {
                    break false;
                }
                fixup_newline(&mut line);
                if line == "#!\n" {
                    break true;
                }
            } {
                break;
            }

            print!("Case {} ---- ", case);

            let mut process = if !python {
                std::process::Command::new(&exe_path)
            } else {
                let mut p = std::process::Command::new(python_command.unwrap());
                p.arg(&file_path);
                p
            }
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();

            let stdin = process.stdin.as_mut().unwrap();
            let stdout = process.stdout.as_mut().unwrap();

            loop {
                let mut line = "".to_owned();
                file.read_line(&mut line).unwrap();
                fixup_newline(&mut line);
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
                fixup_newline(&mut line);
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
            case += 1;

            process.wait().unwrap();
        }

        if !python {
            std::fs::remove_file(exe_path).unwrap();
        }
    }

    println!("Passed / Total: {} / {}", passed, total);
    assert_eq!(passed, total)
}
