# Human command map; npm scripts are the implementation shared with CI.
default:
    @just --list

# Desktop UI in a browser, backed by development-only Rust agents.
dev:
    npm run dev

# Real app using the isolated encrypted demo vault.
native:
    npm run dev:native

desktop:
    npm run dev:desktop

seed:
    npm run dev:seed

# Non-mutating quality gate (tests are separate).
quality:
    npm run format:check
    npm run lint
    npm run typecheck
    npm run check:rust
    npm run check:protocol

# Complete verification, including tests.
check:
    npm run check:all

format:
    npm run format

lint:
    npm run lint

typecheck:
    npm run typecheck

test:
    npm test

test-all:
    npm run test:all

# Select behavior modules (empty arguments runs the full Rust feature inventory).
test-features *features:
    npm run test:features -- {{ features }}

# Core-owned RPC contract and authenticated transport acceptance.
test-rpc:
    npm run test:rpc

# Check local prerequisites without reading vaults or credentials.
doctor:
    node tooling/development/doctor.mjs

welcome:
    bash tooling/development/welcome.sh

# Explicit local opt-in; never installed by npm ci.
hooks-install:
    node tooling/hooks/install.mjs
