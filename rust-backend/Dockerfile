FROM rust:1.95-alpine AS builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM alpine:latest
WORKDIR /app
COPY --from=builder /app/target/release/bare-metal-rust .
COPY --from=builder /app/.env .env.example ./

EXPOSE 8001
CMD ["./bare-metal-rust"]
