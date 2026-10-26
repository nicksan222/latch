# Human command map; Bun scripts are the implementation shared with CI.
default:
    @just --list

# Desktop UI in a browser, backed by development-only Rust agents.
dev:
    bun run dev

# Real app using the isolated encrypted demo vault.
native:
    bun run dev:native

desktop:
    bun run dev:desktop

seed:
    bun run dev:seed

# Non-mutating quality gate (tests are separate).
quality:
    bun run format:check
    bun run lint
    bun run typecheck
    bun run check:rust
    bun run check:protocol

# Complete verification, including tests.
check:
    bun run check:all

format:
    bun run format

lint:
    bun run lint

typecheck:
    bun run typecheck

test:
    bun run test

test-all:
    bun run test:all

# Select behavior modules (empty arguments runs the full Rust feature inventory).
test-features *features:
    bun run test:features -- {{ features }}

# Core-owned RPC contract and authenticated transport acceptance.
test-rpc:
    bun run test:rpc

# Check local prerequisites without reading vaults or credentials.
doctor:
    node tooling/development/doctor.mjs

welcome:
    bash tooling/development/welcome.sh

# Explicit local opt-in; never installed by Bun.
hooks-install:
    node tooling/hooks/install.mjs
