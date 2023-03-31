FROM rust:bookworm AS chef
WORKDIR /app
RUN cargo install cargo-chef --locked

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder 
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release

FROM debian:bookworm AS runtime

RUN apt-get update && apt-get install -y libssl-dev
COPY --from=builder /app/target/release/bk-orchestrator .
CMD ["./bk-orchestrator"]
