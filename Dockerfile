FROM rust:1-bookworm AS server-builder

WORKDIR /app

COPY . .

RUN cargo build --locked --release -p dryrun

FROM debian:bookworm-slim AS server-runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --uid 10001 appuser

WORKDIR /app

COPY --from=server-builder /app/target/release/dryrun /usr/local/bin/dryrun

EXPOSE 8080
EXPOSE 9000

USER appuser

CMD ["dryrun"]

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
