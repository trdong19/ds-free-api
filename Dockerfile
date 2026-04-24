# ---------- 构建阶段 ----------
FROM --platform=$TARGETPLATFORM rust:1.95-slim-bookworm AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY config.example.toml ./

RUN cargo build --release

# ---------- 运行阶段 ----------
FROM --platform=$TARGETPLATFORM debian:bookworm-slim

WORKDIR /app
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/ds-free-api /app/ds-free-api
COPY config.example.toml /app/config.toml

EXPOSE 5317
ENTRYPOINT ["/app/ds-free-api"]
