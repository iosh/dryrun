FROM rust:1-bookworm AS evm-builder

WORKDIR /app/dryrun-evm

COPY dryrun-evm ./

RUN cargo build --locked --release -p dryrun

FROM rust:1-bookworm AS conflux-builder

WORKDIR /app/dryrun-conflux

COPY dryrun-conflux ./

RUN cargo build --locked --release

FROM debian:bookworm-slim AS runtime-base

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --uid 10001 appuser

WORKDIR /app

USER appuser

FROM runtime-base AS evm-runtime

COPY --from=evm-builder /app/dryrun-evm/target/release/dryrun /usr/local/bin/dryrun

EXPOSE 8080
EXPOSE 9000

CMD ["dryrun"]

FROM runtime-base AS conflux-runtime

COPY --from=conflux-builder /app/dryrun-conflux/target/release/dryrun-conflux /usr/local/bin/dryrun-conflux

EXPOSE 8547
EXPOSE 9001

CMD ["dryrun-conflux"]

FROM node:24-alpine AS web-builder

WORKDIR /app/web

RUN corepack enable

COPY web/package.json web/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile

COPY web ./
RUN pnpm build

FROM nginx:1.29-alpine AS web-runtime

COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=web-builder /app/web/dist /usr/share/nginx/html

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
