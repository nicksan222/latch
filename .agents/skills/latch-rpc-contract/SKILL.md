---
name: latch-rpc-contract
description: Change or review Latch RPC methods, messages, enums, and generated clients from the authoritative Protobuf schema.
---

# Latch RPC contract

Use `packages/rpc/proto/latch/v1/agent.proto` as the only authored wire contract. Keep RPCs typed; do not add a generic method-plus-JSON call. Rust and TypeScript clients are generated from this schema.

When changing the contract, update the `.proto`, run `bun run gen:protocol`, and inspect the generated diff. Never hand-edit generated outputs. Run `bun run check:protocol` to verify freshness and the relevant Rust/TypeScript checks for affected consumers.

Core implements the generated service and maps messages to domain types. Apps depend on the RPC interface, not core implementation types. Keep on-disk vault structs separate from protobuf messages; preserve documented camelCase protobuf JSON names and snake_case vault serialization.
