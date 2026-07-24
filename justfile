set shell := ["zsh", "-cu"]

default:
    @just --list

web-dev:
    pnpm -C web dev

web-build:
    pnpm -C web build

web-check:
    pnpm -C web check

server-check:
    cargo check -p dryrun

server-run:
    cargo run -p dryrun

check-server:
    just server-check

check:
    just server-check
    just web-check

compose-up:
    docker compose up --build

compose-down:
    docker compose down
