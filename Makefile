.PHONY := build clean check test install

check:
	@cargo clippy --all-targets --all-features -- -D warnings
	@cargo fmt --all -- --check

build:
	@cargo build --release

clean:
	@cargo clean

test:
	@cargo test --all-features

install:
	@cargo install --path .
