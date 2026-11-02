# BUILD
FROM rust:1.99-alpine3.23 AS builder

WORKDIR /app

# ring (TLS/SSH crypto) compiles C code and links against musl.
RUN apk add --no-cache musl-dev

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release --locked

# RUNTIME
FROM alpine:3.23

WORKDIR /app

RUN apk add --no-cache \
    ca-certificates \
    tzdata

COPY --from=builder /app/target/release/montscan .

RUN mkdir -p /app/storage/inbox /app/storage/ssh

# FTP control + passive range, SFTP, SCP
EXPOSE 21 21000-21010 2222 2223

CMD ["./montscan"]
