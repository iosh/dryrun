FROM rust:1-bookworm AS builder

WORKDIR /app

COPY . .

RUN cargo build --locked --release -p dryrun

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --uid 10001 appuser

WORKDIR /app

COPY --from=builder /app/target/release/dryrun /usr/local/bin/dryrun

EXPOSE 8080
EXPOSE 9000

USER appuser

CMD ["dryrun"]
