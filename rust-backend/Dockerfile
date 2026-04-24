FROM rust:slim-trixie AS builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:trixie-slim
WORKDIR /app
COPY --from=builder /app/target/release/bare-metal-rust .
COPY --from=builder /app/.env .env.example ./

EXPOSE 8001
CMD ["./bare-metal-rust"]
