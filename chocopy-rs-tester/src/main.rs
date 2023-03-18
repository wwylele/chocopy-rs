use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};

fn fixup_newline(s: &mut String) {
    if s.ends_with("\r\n") {
        s.pop();
        s.pop();
        s.push('\n');
    }
}

struct IntegratedCases {
    file: BufReader<File>,
}

impl Iterator for IntegratedCases {
    type Item = (Vec<u8>, Vec<u8>);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let mut line = "".to_owned();
            if self.file.read_line(&mut line).unwrap() == 0 {
                return None;
            }
            fixup_newline(&mut line);
            if line == "#!\n" {
                break;
            }
        }

        let mut input = vec![];
        loop {
            let mut line = "".to_owned();
            self.file.read_line(&mut line).unwrap();
            fixup_newline(&mut line);
            if line == "#<->#\n" {
                break;
            }
            let bytes = line.as_bytes();
            assert!(bytes[0] == b'#');
            input.extend(bytes.iter().skip(1));
        }

        let mut expected_output = vec![];
        loop {
            let mut line = "".to_owned();
            self.file.read_line(&mut line).unwrap();
            fixup_newline(&mut line);
            if line == "#<->#\n" {
                break;
            }
            let bytes = line.as_bytes();
            assert!(bytes[0] == b'#');
            expected_output.extend(bytes.iter().skip(1));
        }

        Some((input, expected_output))
    }
}

fn get_cases(file_path: &std::path::Path) -> Box<dyn Iterator<Item = (Vec<u8>, Vec<u8>)>> {
    let mut ref_path = file_path.to_path_buf();
    ref_path.set_file_name(
        ref_path.file_name().unwrap().to_str().unwrap().to_owned() + ".ast.typed.s.result",
    );

    if let Ok(file) = std::fs::File::open(ref_path) {
        let mut file = BufReader::new(file);
        let mut expected_output = vec![];
        loop {
            let mut line = "".to_owned();
            if file.read_line(&mut line).unwrap() == 0 {
                break;
            }
            fixup_newline(&mut line);
            let bytes = line.as_bytes();
            expected_output.extend(bytes.iter());
        }

        return Box::new(std::iter::once((vec![], expected_output)));
    }

    Box::new(IntegratedCases {
        file: BufReader::new(std::fs::File::open(file_path).unwrap()),
    })
}

#[derive(Debug)]
struct TestFail;

impl std::fmt::Display for TestFail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Output is wrong")
    }
}

impl std::error::Error for TestFail {}

fn test_one_case(
    mut command: std::process::Command,
    input: &[u8],
    expected_output: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut process = command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    let stdin = process.stdin.as_mut().unwrap();
    let stdout = process.stdout.as_mut().unwrap();

    let mut actual_output = vec![];
    stdin.write_all(input)?;
    stdout.read_to_end(&mut actual_output)?;
    process.wait()?;
    if expected_output == &actual_output[..] {
        Ok(())
    } else {
        Err(Box::new(TestFail))
    }
}

fn main() {
    let temp_path = std::env::temp_dir();

    let args: Vec<_> = std::env::args().collect();
    let dir = args.get(1).expect("Path required");
    let option = args.get(2).map(|s| s.as_str());
    let python = option == Some("--python");
    let static_lib = option == Some("--static");
    let python_command;
    if python {
        python_command = Some(args.get(3).map_or("python", |s| s.as_str()));
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

        let mut no_case = true;
        for (case, (input, expected_output)) in get_cases(&file_path).enumerate() {
            no_case = false;
            print!("Case {} ---- ", case);

            let command = if python {
                let mut p = std::process::Command::new(python_command.unwrap());
                p.arg(&file_path);
                p
            } else {
                let mut command = std::process::Command::new(&exe_path);
                if static_lib {
                    command.arg("--static");
                }
                command
            };

            match test_one_case(command, &input, &expected_output) {
                Ok(()) => {
                    println!("\x1b[32mOK\x1b[0m");
                    passed += 1;
                }
                Err(e) => {
                    println!("\x1b[31mError\x1b[0m {}", e);
                }
            }

            total += 1;
        }

        if !python {
            std::fs::remove_file(exe_path).unwrap();
        }

        if no_case {
            panic!("No test case found!");
        }
    }

    println!("Passed / Total: {} / {}", passed, total);
    assert_eq!(passed, total)
}
