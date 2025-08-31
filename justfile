build:
    cargo build --release -p todo-tui

run:
    cargo run -p todo-tui

install: build
    mv target/release/todo ~/.local/bin/todo
