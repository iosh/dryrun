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
    cargo check --manifest-path dryrun-evm/Cargo.toml --workspace

evm-run:
    cd dryrun-evm && cargo run -p dryrun

conflux-check:
    cargo check --manifest-path dryrun-conflux/Cargo.toml

conflux-run:
    cd dryrun-conflux && cargo run

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
