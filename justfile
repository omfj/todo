build:
    cargo run --release

install: build
    mv target/release/todo ~/.local/bin/todo
