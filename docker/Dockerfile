FROM alpine:3.21

RUN apk add --no-cache ca-certificates && \
    adduser -D -h /app -s /sbin/nologin app


WORKDIR /app

COPY target/x86_64-unknown-linux-musl/release/ds-free-api /app/ds-free-api
COPY web/dist /app/web/dist
COPY config.example.toml /app/config.toml

ENV RUST_LOG=info \
    DS_DATA_DIR=/app/data \
    DS_CONFIG_PATH=/app/config.toml

VOLUME /app/data
EXPOSE 5317

USER app

ENTRYPOINT ["/app/ds-free-api"]
