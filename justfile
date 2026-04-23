set shell := ["zsh", "-cu"]

default:
    @just --list

web-dev:
    pnpm -C web dev

web-build:
    pnpm -C web build

web-check:
    pnpm -C web check

compose-up:
    docker compose up --build

compose-down:
    docker compose down
