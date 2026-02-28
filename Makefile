.PHONY: build run check test clean

build:
	cargo build --release

run:
	cargo run

check:
	cargo check

test:
	cargo test

clean:
	cargo clean

fmt:
	cargo fmt

lint:
	cargo clippy -- -D warnings
