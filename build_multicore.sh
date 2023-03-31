#!/bin/sh

rustup override set nightly-2023-03-28;
wasm-pack build --target web --out-dir pkg_multicore -- --config "build-std = [\"panic_abort\", \"std\"]" --features="multicore";
sed -i 's/pivx-shielding/pivx-shielding-multicore/' pkg_multicore/package.json 
rustup override set stable;
