#!/bin/sh

echo "checks that this builds on std+no_std + that all tests run"

cargo build --all-targets # build works
cargo test --all-targets # tests work
# install some no_std target
rustup target add thumbv7em-none-eabihf
# test no_std-build
RUSTFLAGS="-C target-cpu=" cargo build --no-default-features --target thumbv7em-none-eabihf

cargo doc
cargo fmt -- --check
cargo clippy --all-targets
