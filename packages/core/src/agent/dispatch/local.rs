//! Path-bound host operations exposed through the same typed RPC boundary.

use std::path::Path;

use super::Agent;
use crate::core::{Result, Vault, format, git, totp};
use crate::protocol::{
    DoctorReport, EnvParseParams, HookParams, HookResult, OpenParams, TextResult, TotpAlgorithm,
    TotpConfig,
};

pub(super) fn vault_locate(agent: &Agent, input: OpenParams) -> Result<TextResult> {
    if let Ok(top) = git::Git::new(Path::new(&input.path)).work_tree_root()
        && top.join(format::VERSION_FILE).exists()
    {
        return Ok(TextResult {
            text: top.display().to_string(),
        });
    }
    let vault = super::super::open_active_vault(&agent.config_dir)?;
    Ok(TextResult {
        text: vault.path().display().to_string(),
    })
}

pub(super) fn doctor_scan(input: OpenParams) -> Result<DoctorReport> {
    Vault::open(Path::new(&input.path))?.scan(true)
}

pub(super) fn hooks_install(input: HookParams) -> Result<HookResult> {
    let vault = Vault::open(Path::new(&input.path))?;
    let executable = input.executable.as_deref().map(Path::new);
    let path = vault.install_hook(executable)?;
    Ok(HookResult {
        path: Some(path.display().to_string()),
        removed: false,
    })
}

pub(super) fn hooks_uninstall(input: HookParams) -> Result<HookResult> {
    let vault = Vault::open(Path::new(&input.path))?;
    let removed = vault.uninstall_hook()?;
    Ok(HookResult {
        path: None,
        removed,
    })
}

pub(super) fn totp_parse(input: EnvParseParams) -> Result<TotpConfig> {
    let text = input.text;
    let document = if text.trim_start().starts_with("otpauth://") {
        totp::parse_otpauth(&text)?
    } else {
        format::TotpDoc {
            secret: totp::normalize_secret(&text)?,
            algorithm: TotpAlgorithm::Sha1,
            digits: 6,
            period: 30,
            issuer: None,
        }
    };
    Ok(TotpConfig {
        secret: Some(document.secret),
        algorithm: document.algorithm,
        digits: document.digits,
        period: document.period,
        issuer: document.issuer,
    })
}
