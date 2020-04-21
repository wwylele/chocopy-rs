#!/bin/bash
cargo build
cargo test
cargo run --package chocopy-rs-tester -- chocopy-rs/test/pa3
cargo run --package chocopy-rs-tester -- chocopy-rs/test/original/pa3
