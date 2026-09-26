.PHONY: check fix test install build clean

check:
	@cargo clippy --all-targets --all-features -- -D warnings
	@cargo fmt --all -- --check

fix:
	@cargo fmt --all
	@cargo clippy --all-targets --all-features --fix --allow-dirty --allow-staged

test:
	@cargo test --all-targets --all-features

install:
	@cargo install --path todo-tui

build:
	@cargo build --release

clean:
	@cargo clean
