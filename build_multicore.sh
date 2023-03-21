#!/bin/sh

rustup override set nightly;
wasm-pack build --target web -- --config "build-std = [\"panic_abort\", \"std\"]" --features="multicore";
rustup override set stable;
