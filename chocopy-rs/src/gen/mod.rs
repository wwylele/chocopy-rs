use faerie::*;
use std::convert::*;
use std::io::Write;
use std::str::FromStr;
use target_lexicon::*;

pub fn gen(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let obj_name = format!("chocopy-{}.o", rand::random::<u32>());
    let mut obj_path = std::env::temp_dir();
    obj_path.push(obj_name.clone());

    let mut obj = ArtifactBuilder::new(triple!("x86_64-pc-linux-gnu-elf"))
        .name(obj_name)
        .finish();

    obj.declarations(
        [
            ("chocopy_main", Decl::function().global().into()),
            ("debug_print", Decl::function_import().into()),
        ]
        .iter()
        .cloned(),
    )?;

    obj.define(
        "chocopy_main",
        vec![
            0x55, 0x48, 0x89, 0xe5, 0xbf, 0x2a, 0x00, 0x00, 0x00, 0xe8, 0x00, 0x00, 0x00, 0x00,
            0x5d, 0xc3,
        ],
    )?;

    obj.link(Link {
        from: "chocopy_main",
        to: "debug_print",
        at: 10,
    })?;

    let obj_file = std::fs::File::create(&obj_path)?;
    obj.write(obj_file)?;

    let ld_output = std::process::Command::new("ld")
        .args(&[
            "-o",
            path,
            "-l:crt1.o",
            "-l:crti.o",
            "-l:crtn.o",
            obj_path.to_str().unwrap(),
            "target/debug/libchocopy_rs_std.a",
            "-lc",
            "-lpthread",
            "-ldl",
            "--dynamic-linker=/lib64/ld-linux-x86-64.so.2",
        ])
        .output()?;

    println!("ld status: {}", ld_output.status);
    std::io::stdout().write_all(&ld_output.stdout).unwrap();
    std::io::stderr().write_all(&ld_output.stderr).unwrap();

    std::fs::remove_file(&obj_path)?;

    Ok(())
}
