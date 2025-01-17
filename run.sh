#!/usr/bin/env sh

commands() {
    RUST_LOG=hackathon=debug cargo test test_api -- --nocapture
    RUST_LOG=hackathon=debug cargo run -- --help

    RUST_LOG=hackathon=debug cargo run -- code 'write a hello world app in rust'
    RUST_LOG=hackathon=debug cargo run -- chat 'how do you write a hello world app in rust'

    RUST_LOG=hackathon=debug cargo run -- how do you write a hello world app in rust
    RUST_LOG=hackathon=debug cargo run -- what is the purpose of a hello world app
}

