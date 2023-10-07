#!/bin/sh
set -e
cargo run --release ghdeps.toml ghversions.toml
cd .github/actions/selfupdate
cargo run --release ../../../ghdeps.toml ../../../ghversions.toml ../../../Cargo.toml ./Cargo.toml
cd ../../..
