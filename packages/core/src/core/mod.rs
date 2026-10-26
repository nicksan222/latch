//! Vault core. No UI, no IPC.
//!
//! Entry points: [`Vault`] (a located, locked vault) and [`UnlockedVault`]
//! (profiles, entries, history, env, TOTP, recipients, doctor, sync). Methods
//! take and return `crate::protocol` types so the agent's RPC dispatcher can
//! call them directly. Errors are [`Error`], mapped to wire codes with
//! [`Error::code`].
//!
//! Module map:
//! - `paths`      per-user dirs with `LATCH_*` overrides
//! - `ids`        random 26-char base32 ids
//! - `crypto`     Argon2id, age, HKDF, Ed25519, BLAKE3, hardening
//! - `format`     on-disk documents (snake_case), layout, generated files
//! - `secrets`    entry CRUD, reveal, TOTP orchestration, secret-kind behavior
//! - `vault`      lifecycle, integrity, authenticated storage, write transactions
//! - `profiles`   profile CRUD and metadata
//! - `environment` profile environment selection, resolution and formatting
//! - `history`    entry history, restore, trash
//! - `recipients` recipients, rotation, passphrase change
//! - `doctor`     integrity checks, plaintext scan, pre-commit hook
//! - `sync`       fetch/rebase/push with conflict resolution
//! - `trust`      local trust anchor (pinned key history), keychain, consent
//! - `git`        system `git` wrapper
//! - `totp`       OTP primitives

pub mod actions;
pub mod browser;
pub mod crypto;
pub mod doctor;
pub mod domain;
pub mod environment;
pub mod error;
pub mod export;
pub mod format;
pub mod git;
pub mod history;
pub mod ids;
pub mod import;
pub mod paths;
mod profiles;
pub mod recipients;
pub mod secrets;
pub mod sync;
pub mod totp;
pub mod trust;
pub mod vault;

pub use error::{Error, Result};
pub use trust::{Consent, TrustInfo, TrustState};
pub use vault::{Credentials, IntegrityProblem, UnlockedVault, Vault};
