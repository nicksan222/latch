---
name: latch-release
description: Prepare, review, or troubleshoot Latch CLI and desktop release builds and version publication.
---

# Latch release

Release planning and publication live in `tooling/release`; Rust packaging lives in `tooling/automation/src/release.rs`. GitHub workflows own CI and publication. Read the relevant workflow and scripts before changing release behavior.

Keep release planning deterministic and scoped to the requested version. The planner does not create tags or edit source files; version preparation operates on a disposable checkout. Preserve stable semver validation, main-branch ancestry checks, and serialized publication behavior.

Use focused release checks with `bun run test:quality` (includes `tooling/release/*.test.mjs`). Do not report a build or publication as successful without inspecting its actual result. Publishing, tagging, or pushing requires the user's explicit request.
