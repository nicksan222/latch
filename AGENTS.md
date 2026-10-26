# Latch agent guide

- Read `README.md` for product scope and security limits.
- Use `.agents/skills/latch-rpc-contract/` for RPC contract work, `latch-managed-browser/` for managed Firefox, and `latch-release/` for release workflows.
- `packages/rpc/proto/latch/v1/agent.proto` is the sole RPC contract. Regenerate with `npm run gen:protocol`; never hand-edit generated outputs.
- Never write plaintext under a vault path or put secrets in logs, errors, panics, debug output, commits, or non-TTY CLI output. Fail closed on integrity errors.
- Rust core owns domain behavior; apps consume the generated RPC interface. Do not add another wire contract or a generic JSON RPC.
- Keep secrets out of the frontend except after explicit reveal/edit actions; clear plaintext on navigation or reveal timeout. Clipboard writes go through `clipboard.copy`.
- Rust without Tauri: use `--no-default-features`. Keep KDF minimums at 64 MiB outside tests or `LATCH_ALLOW_WEAK_KDF=1`.
- Core import/export/secret kinds register locally in their family; do not add central dispatch matches or registry arrays.
- Run focused checks while working. Main gates: `npm run check:all` and `npm run test:all`; native desktop checks require the devcontainer (`npm run check:desktop`). Do not claim CI passed without checking its run.
- Keep UI primitives in `packages/ui` and app features in their apps. Keep documentation in the root README; agent skills are the automation entrypoints.
