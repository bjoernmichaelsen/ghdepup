#!/bin/sh
cargo run --release ghdeps.toml ghversions.toml
cd .github/actions/selfupdate
cargo run --release ../../ghversions.toml
