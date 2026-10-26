# Latch

Local, Git-backed secrets for developers. One encrypted vault, a desktop app, a CLI, and isolated project browsers. No account or hosted service.

![Unlock a vault, browse a project and copy a credential](.github/assets/demo.gif)

- Encrypted profiles, logins, API keys, environment variables, history and sharing.
- Managed Firefox with workspace logins in its native password manager; one-way sync from Latch.
- One Rust core; desktop and CLI share the [Protobuf interface](packages/rpc/proto/latch/v1/agent.proto).

[**Download Linux x86_64**](https://github.com/nicksan222/latch/releases) · [Report a vulnerability privately](https://github.com/nicksan222/latch/security/advisories/new) · [MIT](LICENSE)

Linux packages require glibc 2.36+. macOS builds from source. CI covers Linux x86_64/arm64 and Apple Silicon macOS.

## Develop

Node 24+, Bun 1.4.4, and the pinned Rust toolchain. Use the Dev Container for native Linux dependencies; macOS needs Xcode Command Line Tools.

```sh
bun install --frozen-lockfile
bun run dev          # Isolated demo in your browser
bun run desktop:dev  # Native app
bun run demo:record  # Regenerate README GIF (Docker)
bun run cli -- status
bun run check:all    # Full quality gate
```

Edit `.proto`, then `bun run gen:protocol`. Project agent skills live in [`.agents/skills`](.agents/skills).

**Early release, not independently security-audited.** Keep backups. Git exposes metadata; removed recipients retain old history. Same-user malware can access unlocked secrets. Browser isolation is not an OS security boundary, and browser updates are maintainer-controlled.

## Container test implementation plan

Planning baseline: 2026-10-26. The commands below describe the intended result; they are not available until their PR lands. Keep this temporary plan here to follow the repository's root-documentation convention; replace it with concise command documentation as the work ships.

> **For agentic workers:** Use `superpowers:executing-plans` to implement each PR task by task. Use `superpowers:subagent-driven-development` only when delegation is selected. Checkboxes track implementation, not planning completion.

**Goal:** Assume development takes place in the devcontainer and leave three everyday commands: `bun run dev`, `bun run test`, and `bun run check`. Run Linux Firefox, native desktop, and frontend acceptance tests with dependencies defined in Dockerfiles and container lifecycle owned by Rust Testcontainers; select an individual suite with `bun run test -- browser`, `-- desktop`, or `-- e2e`.

**Architecture:** A shared multi-stage test Dockerfile supplies separate Firefox, desktop, and E2E targets. A host Rust runner builds the selected target and runs a fixed suite inside it; Rust fixtures and ordinary test harnesses own individual scenarios. Native macOS acceptance stays on macOS. Testcontainers is the container transport, not another test assertion framework.

**Tech stack:** Rust 1.98.1, Rust Testcontainers, Docker, Node 24+, Bun 1.4.4, Cargo tests, and the existing Playwright runner.

**Design basis:** Shared dependency stages with separate test targets, approved in this conversation. The PR boundaries, feature names, interfaces, and build-context policy below are engineering decisions for implementation. Existing behavior is in `tooling/automation/src/native_browser.rs`, `tooling/automation/src/rust_checks.rs`, `tooling/e2e/`, and `.github/workflows/test-reusable.yml`.

### Constraints and execution contract

- Preserve the exact pinned, checksum-verified app-managed Firefox. Do not substitute Playwright Firefox or a system browser. Keep the existing Marionette adapter unless a separate change proves equivalent coverage with less complexity.
- Keep domain behavior in Rust core and external RPC scenarios on the generated Protobuf client. No new product wire contract.
- Use synthetic credentials and isolated temporary profiles. Never copy real vaults, user configuration, credentials, decrypted session state, or host environment secrets into build contexts, images, logs, or artifacts. Preserve KDF minimums of 64 MiB outside tests or `LATCH_ALLOW_WEAK_KDF=1`.
- Development always runs in the repository devcontainer. Assume its pinned Rust/Bun/Node toolchains and Docker-outside-of-Docker access; do not build host installation discovery, fallback launchers, or alternate bare-host development flows. CI bootstrap and native macOS runners are explicit platform adapters, not developer setup choices. Image builds compile Linux binaries; do not copy a host executable into a Linux image.
- No Docker socket inside test containers, privileged mode, host networking, or writable checkout mounts. Containers run as a non-root test user; `/dev/shm` is 1 GiB. Profiles and mutable test state are per invocation.
- Add `containers` and `browser-acceptance` as opt-in automation features. Ordinary headless workspace tests must not require Docker or download/launch Firefox. Keep native desktop builds behind their existing native dependency boundary.
- The host CLI is `latch-automation test-container <browser|desktop|e2e>` with the `containers` feature. The container calls the native suite directly, never the host orchestration command. Introduce suite commands during migration, then converge in PR 5 on `bun run test` with optional `-- browser|desktop|e2e|unit` selection. Document only `dev`, `test`, and `check` as everyday commands; retain `gen:protocol` as a specialist command.
- Container execution uses argument vectors, a bounded suite deadline, actual process completion and exit status, and streamed safe diagnostics. A log message alone cannot establish that tests passed. No arbitrary sleeps for readiness.
- Use a temporary allowlisted source snapshot containing current local edits, with relative paths preserved. Exclude `.git`, `.dev`, dependency/build outputs, vaults, and untracked files outside the explicit source inputs. Include manifests, locks, patches, build scripts, generated inputs, frontend assets, and `rust-toolchain.toml` required by each target. Reuse Docker layer cache, not mutable test state. Source changes must invalidate the appropriate build layers.
- Test image distribution and dependency versions must match the repository pins. Resolve and lock an actual compatible Testcontainers crate version during PR 2; verify build-target, exec, timeout, artifact-copy, and cleanup APIs against that version rather than assuming website snippets compile.
- Keep `.devcontainer/Dockerfile` and release packaging unchanged initially. Sharing the interactive workstation or release image is a later decision, not a prerequisite for isolated test images.

### Review focus

1. Missing Firefox/cache or an empty test selector must fail or provision explicitly; never pass without executing assertions. PRs 1 and 2 own these regressions.
2. A nonzero child exit, timeout, SIGINT, or SIGTERM must fail the command and remove owned containers. PR 2 verifies these against a real Docker daemon; document that SIGKILL/daemon failure cannot rely on Rust `Drop`.
3. Dirty source changes must reach the container while excluded synthetic sentinel files never enter the build context. PR 2 tests both context behavior and image invalidation.
4. Concurrent invocations must not share profiles, host ports, output directories, or writable caches. PRs 2 and 4 exercise concurrency.
5. Linux container success must not replace native macOS coverage or accidentally claim GUI coverage from compilation alone. PRs 3 and 5 retain and verify the platform matrix.

### PR 1 — Readable Firefox acceptance tests and honest failures

**Depends on:** nothing. Preserve the current native command and CI setup while improving the suite.

**Files:** modify `tooling/automation/Cargo.toml`, `tooling/automation/src/native_browser.rs`, `tooling/automation/src/native_browser/marionette.rs` only if extraction requires it, `packages/core/src/core/browser/tests/browser_sessions.rs`, and `packages/core/src/rpc/tests/external/native_browser.rs`; create `tooling/automation/tests/native_browser.rs` and `tooling/automation/src/native_browser/fixture.rs`; update `tooling/quality/features.json` for moved acceptance paths.

**Interfaces:** retain `native_browser::run() -> anyhow::Result<()>` for the CLI. Add a native acceptance test target requiring `browser-acceptance`; its tests use a shared `BrowserFixture` with `start() -> Result<Self>`, `seed_login(username: &str, password: &str) -> Result<()>`, `navigate_to_login() -> Result<()>`, and `assert_password(expected: &str) -> Result<()>`. Keep origin/invalid-batch/ownership helpers specific to those scenarios. The fixture owns its server, profile, Firefox process, and bounded readiness waits. Do not put generic preflight checks in scenario bodies.

- [ ] Add a regression proving that explicitly selected real-browser acceptance cannot succeed with no runtime and no assertions. Remove the cache-dependent successful return in `real_managed_firefox_smoke`; migrate its session archive assertions into the maintained native suite and remove the redundant smoke test.
- [ ] Add separately named tests for matching-origin autofill, other-origin isolation, atomic malformed-batch rejection, update, reopen/update, deletion, partial-failure recovery, and browser-owned credential protection. Give each independent profile/state; share only the verified immutable runtime cache.
- [ ] Extract the current implementation behind those tests, preserve scenario assertions, and have the CLI invoke the exact named test target plus the generated-client RPC acceptance. Refuse empty discovery for filtered RPC acceptance. Forward useful safe provisioning/launch diagnostics without dumping credential-bearing payloads or profile contents.
- [ ] Run `cargo test --locked -p latch-automation --no-default-features` and `bun run test:quality`; run `bun run test:browser:native` with the existing Linux Xvfb setup and on native macOS. Confirm individual test names appear and each existing security assertion is accounted for.
- [ ] Review the fixture cleanup on assertion failure, commit, and open PR 1. Record commands actually run and platform limitations in the PR.

### PR 2 — Test Dockerfile and Rust-owned Firefox execution

**Depends on:** PR 1. Deliver the first complete containerized suite, not unused container infrastructure.

**Files:** create `tooling/automation/containers/Dockerfile`, `tooling/automation/src/containers/mod.rs`, `tooling/automation/src/containers/context.rs`, `tooling/automation/src/containers/tests.rs`, and `tooling/automation/tests/containers.rs`; modify `tooling/automation/Cargo.toml`, `Cargo.lock`, `tooling/automation/src/lib.rs`, `tooling/automation/src/main.rs`, and `package.json`.

**Interfaces:** `containers::Suite { Browser }`, `containers::run(suite: Suite) -> anyhow::Result<()>`, and `containers::context::snapshot(root: &Path, destination: &Path) -> anyhow::Result<()>`. The CLI requires the `containers` feature for its `test-container` subcommand. Dockerfile targets are `test-base` and `browser-tests`; later PRs extend `Suite` and add targets. The browser worker invokes PR 1's native acceptance under container-owned Xvfb/DBus. Keep the host runner separate from that worker so it cannot recurse.

- [ ] Resolve Testcontainers version/features and prove the selected library can build a named Dockerfile target from the explicit context, run it, retrieve completion status, and copy allowlisted artifacts. Fail clearly if Docker is unavailable; do not silently skip. Make any necessary compatibility decision explicit in this PR before adding another orchestration layer.
- [ ] Add failing context tests: include a modified Rust source and required manifests/patches, exclude synthetic vault/credential/build-output sentinels, reject symlink escapes, and preserve relative paths. Add CLI tests for accepted suites and invalid selections.
- [ ] Implement the shared dependency stage and Firefox target. Install dependencies at image build time and provision Firefox through the product installer with its existing pin/checksum checks. Layer dependencies ahead of frequently changing sources. Validate the pinned distribution's library package names on both Linux architectures.
- [ ] Implement build, start, command completion, timeout, logs, and cleanup in Rust using the library. Register SIGINT/SIGTERM handling that explicitly awaits cleanup; do not assume `Drop` runs on process termination. Use library-supported process-death cleanup if available and verified; state the hard-kill limitation otherwise. Preserve the original test error if cleanup also fails.
- [ ] Add `bun run test:browser` dispatching to `test-container browser`. Keep `test:browser:native` for native platform verification. Verify success, controlled exit 7, timeout, SIGINT/SIGTERM, two concurrent runs, changed-source rebuild, and empty/corrupt Firefox cache against Docker. Verify no owned container remains after handled failures and no excluded context sentinel appears in the image.
- [ ] Run the Firefox command from an environment without host Firefox/Xvfb/Tauri libraries. Confirm it builds Linux artifacts inside the image and the named scenarios really execute. Run automation unit tests and `bun run test:quality`, review, commit, and open PR 2.

### PR 3 — Native Linux desktop tests through the same runner

**Depends on:** PR 2. Add one suite without introducing a second lifecycle manager.

**Files:** modify `tooling/automation/containers/Dockerfile`, `tooling/automation/src/containers/mod.rs`, `tooling/automation/src/containers/tests.rs`, `tooling/automation/tests/containers.rs`, `tooling/automation/src/rust_checks.rs`, and `package.json`.

**Interfaces:** extend `Suite` with `Desktop`; add the Dockerfile `desktop-tests` target extending the dependency base. `bun run test:desktop` executes `test-container desktop`. Keep `bun run check:desktop` as the native-host Rust quality gate; its current scope is `rust all --desktop`, not `desktop_ci()`.

- [ ] Add a test asserting that the desktop suite selects the desktop target and never selects browser/E2E orchestration. Add a controlled failing desktop test to verify failure propagation during development, then remove the deliberate failure before committing.
- [ ] Install WebKitGTK/Tauri dependencies in the desktop stage. Run frozen Bun installation, frontend build, native desktop build, and existing desktop Rust quality/tests with Xvfb and DBus inside the container. Reuse `rust_checks::desktop_ci()` where its behavior fits; keep compilation and test execution visible as distinct reported steps.
- [ ] Verify a clean run without host WebKitGTK/Xvfb, assertion failure cleanup, and absence of writable checkout mounts. State exactly which existing native assertions execute; do not describe a successful build as an end-to-end desktop interaction test.
- [ ] Run `bun run test:desktop`, automation unit tests, and `bun run test:quality`; review, commit, and open PR 3. Keep the existing native macOS desktop job intact.

### PR 4 — Self-contained frontend E2E environment

**Depends on:** PR 2; may be prepared separately from PR 3 after the shared runner interface is stable.

**Files:** modify `tooling/automation/containers/Dockerfile`, `tooling/automation/src/containers/mod.rs`, `tooling/automation/tests/containers.rs`, `tooling/e2e/playwright.config.ts`, `tooling/e2e/prerequisites.ts`, `tooling/e2e/prerequisites.test.mjs`, and `package.json`. Preserve app-owned scenarios in `apps/desktop/tests/e2e/` and existing fixture isolation.

**Interfaces:** extend `Suite` with `E2e`; add target `e2e-tests`. `bun run test:e2e` dispatches to `test-container e2e`; expose the existing `node tooling/e2e/run.mjs` path as `bun run test:e2e:native` for the container worker and explicit local debugging. The worker must never invoke the public container command.

- [ ] Add a failing test for E2E suite dispatch and artifact routing. Keep inner loopback port 1423: separate container network namespaces already isolate it, so do not add a redundant host port allocator or publish this port.
- [ ] Install the lockfile-selected Playwright Chromium and its Linux dependencies in the E2E image. Run the existing Playwright controller, frontend launcher, and Rust bridge inside the same container. Preserve existing process ownership and health waits until evidence justifies removing a wrapper.
- [ ] Export only the current invocation's allowlisted synthetic-test traces and reports to `.dev/test-results/run-<id>` before container removal. Ensure artifact export still happens on suite failure, cannot traverse out of its destination, and does not replace the original failure. Do not export vault/profile directories.
- [ ] Run `bun run test:e2e` twice concurrently with default settings. Confirm both pass without host port conflicts, with separate artifacts and sessions. Induce an assertion failure, verify its trace is available and cleanup completes, then remove the deliberate failure.
- [ ] Run E2E prerequisite/runner tests and automation tests, review, commit, and open PR 4.

### PR 5 — Align CI, complete gates, and developer commands

**Depends on:** PRs 2–4. Switch CI only after each suite is independently usable.

**Files:** modify `.github/workflows/test-reusable.yml`, `package.json`, `bun.lock`, `.github/pull_request_template.md`, and this README; create `tooling/automation/src/test_suites.rs`; remove `Justfile` and its `rust-just` dependency/format hook once all recipes have canonical Bun replacements. Modify quality tests only for meaningful command-selection or inclusion behavior, not snapshots of script strings.

**Interfaces:** `bun run test` runs every supported test suite; `bun run test -- browser|desktop|e2e|unit` selects one suite through `test_suites::run(selection: Option<TestSuite>) -> anyhow::Result<()>`. `bun run check` is the complete quality gate including tests. Keep the existing `bun run dev` default and offer native desktop launch as `bun run dev -- desktop`; existing specialist tooling can remain implementation details. Remove redundant `test:all`/`check:all`, per-suite migration aliases, welcome/workflow aliases, and the Justfile command map once callers are migrated. Native Firefox/desktop/E2E worker commands remain internal, explicit, and never recurse into the public container runner. Update AGENTS.md references if its gate names change.

- [ ] Map every existing CI check to its successor command before editing jobs. Preserve Rust/protocol checks, secret scanning, frontend checks, native macOS Firefox/desktop coverage, and native Linux arm64 Firefox/session coverage. Running the arm64 container suite on an arm64 runner remains native Linux execution; do not substitute amd64 emulation.
- [ ] Replace Linux test-specific package-install/Xvfb blocks and the desktop job's handwritten `docker build`/`docker run` with the corresponding public suite commands. Install only the prerequisites needed by the host Rust runner. Keep necessary package installation for separate non-container Rust jobs; do not remove it merely because test images exist.
- [ ] Implement the small test selector and devcontainer-only command surface. Test default all-suite inclusion, each valid selector, rejection of typos/extra arguments, and no recursion from workers. Keep fast unit tests available through `bun run test -- unit`; do not add machine prerequisite probing. Make CI run the same individual suite entrypoints as local development. Keep per-suite jobs and artifact upload paths inspectable; each job installs the pinned host toolchains/Bun dependencies before invoking its command. Avoid repeating suites in CI by also invoking an aggregate there.
- [ ] Run `bun run check` and `bun run test` in the devcontainer with Docker access, and record their cost. Verify an unavailable daemon produces an actionable failure rather than a successful partial gate. Confirm the unit selector does not start containers. Remove stale command references from README, AGENTS.md, hooks, CI, and automation skills as part of the same PR.
- [ ] Verify Linux x86_64, Linux arm64, and native macOS workflow runs on the actual PR. Update the README's command prerequisites and PR checklist to match observed coverage. Replace this temporary plan with the shipped command documentation only after its final tasks are complete.
- [ ] Review command inclusion, image architectures, secrets/context boundaries, and cancellation behavior; resolve meaningful findings, commit, and open PR 5. Report actual CI status rather than claiming success from local checks.

### PR delivery and later cleanup

Deliver PR 1 first, then PR 2. PRs 3 and 4 both build on PR 2; execute sequentially in the initial stack to avoid shared runner/Dockerfile conflicts. PR 5 targets the completed stack. Suggested branches: `test/firefox-scenarios`, `test/firefox-containers`, `test/desktop-containers`, `test/e2e-containers`, and `test/container-gates`. Every PR must work against its declared base and carry its own verification; prefer small stacked diffs and merge in dependency order. Link each created PR to the working thread. Do not create or publish PRs merely to record this plan.

Two follow-up PRs should stay separate: consolidate the Bash/Python development environment policy while preserving seed isolation; and assess reusing the container lifecycle for demo recording, including output extraction, intentional pacing, retention options, and interruption cleanup. Neither is necessary to prove containerized tests. Keep release packaging out of this migration.

References: [Rust Testcontainers image builds](https://rust.testcontainers.org/features/building_images/), [container command execution](https://rust.testcontainers.org/features/exec_commands/), and [lifecycle basics](https://rust.testcontainers.org/quickstart/testcontainers/). Confirm capabilities against the crate version locked in PR 2.
