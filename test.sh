#!/bin/bash
cargo build
cargo test
cargo run --package chocopy-rs-tester -- chocopy-rs/test/pa3
