FROM rust:bookworm AS builder

COPY . .
#run apt-get update && apt-get install -y libssl-dev
RUN cargo build --release

FROM debian:bookworm

COPY --from=builder ./target/release/bk-orchestrator .
CMD ["./bk-orchestrator"]
