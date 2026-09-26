.PHONY := check test install build clean

check:
	@cargo clippy --all-targets --all-features -- -D warnings
	@cargo fmt --all -- --check

test:
	@cargo test --all-targets --all-features

install:
	@cargo install --path .

build:
	@cargo build --release

clean:
	@cargo clean
