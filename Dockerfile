# BUILD
FROM rust:1.100-alpine3.24 AS builder

WORKDIR /app

# ring (TLS/SSH crypto) compiles C code and links against musl.
RUN apk add --no-cache musl-dev

# Build dependencies alone first, against a placeholder main.rs, so this layer
# stays cached until Cargo.toml or Cargo.lock change.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
    && echo 'fn main() {}' > src/main.rs \
    && cargo build --release --locked \
    && rm -rf src

# Then the app itself; touch so cargo rebuilds it over the placeholder.
COPY src ./src
RUN touch src/main.rs && cargo build --release --locked

# RUNTIME
FROM alpine:3.24

WORKDIR /app

RUN apk add --no-cache \
    ca-certificates \
    tzdata

COPY --from=builder /app/target/release/montscan .

RUN mkdir -p /app/storage/inbox /app/storage/ssh

# FTP control + passive range, SFTP, SCP
EXPOSE 21 21000-21010 2222 2223

CMD ["./montscan"]
