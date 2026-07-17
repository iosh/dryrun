set shell := ["zsh", "-cu"]

default:
    @just --list

web-dev:
    pnpm -C web dev

web-build:
    pnpm -C web build

web-check:
    pnpm -C web check

evm-check:
    cargo check -p dryrun

evm-run:
    cd apps/dryrun-evm && cargo run -p dryrun

conflux-check:
    cargo check -p dryrun-conflux

conflux-run:
    cd apps/dryrun-conflux && cargo run -p dryrun-conflux

check-server:
    just evm-check
    just conflux-check

check:
    just evm-check
    just conflux-check
    just web-check

compose-up:
    docker compose up --build

compose-down:
    docker compose down
