.PHONY: all
all: pkg pkg_multicore js/pivx_shield.js js/README.md

pkg: src/ Cargo.toml
	wasm-pack build --target web
	sed -i 's/pivx_shield_rust_bg.wasm/*/' pkg/package.json
	cp wrong-package.md pkg/README.md

pkg_multicore: src/ Cargo.toml
	RUSTFLAGS='-C target-feature=+atomics,+bulk-memory,+mutable-globals' \
	  rustup run nightly-2023-03-28 \
	  wasm-pack build --weak-refs --out-dir "pkg_multicore" --target web -- --features="multicore" -Z build-std=panic_abort,std
	sed -i 's/pivx-shield-rust/pivx-shield-rust-multicore/' pkg_multicore/package.json
	sed -i 's/pivx_shield_rust_bg.wasm/*/' pkg_multicore/package.json
	cp wrong-package.md pkg_multicore/README.md

js/README.md: README.md
	cp README.md js/

js/pivx_shield.js: js/pivx_shield.ts js/node_modules
	cd js/; \
	npm run build

js/node_modules: js/package.json
	cd js/; (npm ci || npm i)

.PHONY: publish
publish: all
	cd pkg; npm publish
	cd pkg_multicore; npm publish
	cd js; npm publish

.PHONE: pack
pack: all
	cd pkg; npm pack
	cd pkg_multicore; npm pack
	cd js; npm pack

.PHONY: clean
clean:
	cargo clean
	-rm -rf pkg/
	-rm -rf pkg_multicore/
	-rm -rf js/node_modules
	-rm js/pivx_shield.d.ts

