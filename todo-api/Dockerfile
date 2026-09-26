FROM rust:1-bookworm AS builder

WORKDIR /app
COPY . .

RUN cargo build --release -p todo-api

FROM debian:bookworm-slim AS runtime

RUN useradd --create-home --uid 10001 app \
    && mkdir -p /data \
    && chown app:app /data

COPY --from=builder /app/target/release/todo-api /usr/local/bin/todo-api

USER app
EXPOSE 3000

ENV DATABASE_URL=sqlite:///data/todo.db?mode=rwc
ENV RUST_LOG=todo_api=info,tower_http=info

CMD ["todo-api"]
