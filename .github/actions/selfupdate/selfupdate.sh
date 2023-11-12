#!/bin/sh
cargo run --release ghdeps.toml ghversions.toml
cd .github/actions/selfupdate
cargo run --release ../../../ghversions.toml
cd ../../..
pwd && ls -a
/usr/bin/git config --global --add safe.directory /github/workspace
git diff
git commit -m "update dependencies"
