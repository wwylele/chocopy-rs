# ChocoPy compiler written in Rust

[![Build](https://github.com/wwylele/chocopy-rs/workflows/Rust/badge.svg)](https://github.com/wwylele/chocopy-rs/actions)

> [ChocoPy](https://chocopy.org/) is a programming language designed for classroom use in undergraduate compilers courses. ChocoPy is a restricted subset of Python 3, which can easily be compiled to a target such as RISC-V. The language is fully specified using formal grammar, typing rules, and operational semantics. ChocoPy is used to teach CS 164 at UC Berkeley. ChocoPy has been designed by Rohan Padhye and Koushik Sen, with substantial contributions from Paul Hilfinger.

So this was a course project for me. But I also want to try something else (plus 2020 March-April was not a good time to go outside LOL), so here it is, a second implementation that
 - is written in ~~Java~~ Rust,
 - has a parser ~~built using JFlex and CUP~~ hand-written,
 - targets ~~RISC-V~~ x86-64, and
 - produces ~~assemblies that runs in a simluator~~ executables that runs directly in Windows, Linux or macOS.
   - I haven't thoroughly tested macOS support. It just passes CI tests.

This project is licensed under [MIT License](LICENSE). Test case files under [`chocopy-rs/test/original`](chocopy-rs/test/original) are from the original course project, with their own copyright notice in the directory.

See [DESIGN.md](DESIGN.md) for some design detail.

## Build

Requirement: basic Rust environment (`rustc`, `cargo` etc.). Tested version: 1.42

```bash
cargo build
```

This will produce two binaries in the target directory:
 - `chocopy-rs` (Linux, macOS) / `chocopy-rs.exe` (Windows)
 - `libchocopy_rs_std.a` (Linux, macOS) / `chocopy_rs_std.lib` (Windows)

 They should always be placed in the same directory.

## Runtime Dependency

chocopy-rs needs a linker and some basic libraries to produce native executable:
 - Windows: Visual Studio (full version or build tools only), Windows SDK
   - chocopy-rs invokes command `link.exe` with Visual Studio environment set up
 - Linux / macOS: ld (with GCC or Clang as interface), libc
   - chocopy-rs invokes command `cc`

These are not required if you only use chocopy-rs to produce AST JSON or object file.

## Usage

```bash
# compile source file input.py to executable output.exe
chocopy-rs input.py output.exe

# same as above, but link against static library
chocopy-rs input.py output.exe --static

# compile source file input.py to object file output.o
chocopy-rs input.py output.o --obj

# same as above, but genernates the object file in ELF format for linux
chocopy-rs input.py output.o --obj --platform linux

# parse source file and output untyped AST JSON to STDOUT
chocopy-rs input.py --ast

# parse and check source file and output typed AST JSON to STDOUT
chocopy-rs input.py --typed

```

## Debugging

The compiler also generates debugging information for ChocoPy programs:
 - A separate `*.pdb` file for Windows
 - DWARF embedded in the program for Linux / macOS

With debugging information, ChocoPy programs can be debugged with common tools:
 - Windows: WinDbg / Visual Studio Windows Debugger
 - Linux: GDB / LLDB
 - macOS: I haven't tested it but maybe LLDB works
