//! Explicit, checked conversions at the protobuf boundary.
use super::models as model;
use crate::core::{Error, Result};
use latch_rpc::v1;
fn invalid(field: &str) -> Error {
    Error::InvalidParams(format!("invalid or missing {field}"))
}
impl TryFrom<i32> for model::DomainRecipientKeyKind {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match v1::DomainRecipientKeyKind::try_from(value) {
            Ok(v1::DomainRecipientKeyKind::Age) => Ok(Self::Age),
            Ok(v1::DomainRecipientKeyKind::Ssh) => Ok(Self::Ssh),
            Ok(v1::DomainRecipientKeyKind::Plugin) => Ok(Self::Plugin),
            _ => Err(invalid("DomainRecipientKeyKind")),
        }
    }
}
impl From<model::DomainRecipientKeyKind> for v1::DomainRecipientKeyKind {
    fn from(value: model::DomainRecipientKeyKind) -> Self {
        match value {
            model::DomainRecipientKeyKind::Age => Self::Age,
            model::DomainRecipientKeyKind::Ssh => Self::Ssh,
            model::DomainRecipientKeyKind::Plugin => Self::Plugin,
        }
    }
}
impl From<model::DomainRecipientKeyKind> for i32 {
    fn from(value: model::DomainRecipientKeyKind) -> Self {
        v1::DomainRecipientKeyKind::from(value) as i32
    }
}
impl TryFrom<i32> for model::ClientKind {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match v1::ClientKind::try_from(value) {
            Ok(v1::ClientKind::Cli) => Ok(Self::Cli),
            Ok(v1::ClientKind::Desktop) => Ok(Self::Desktop),
            _ => Err(invalid("ClientKind")),
        }
    }
}
impl From<model::ClientKind> for v1::ClientKind {
    fn from(value: model::ClientKind) -> Self {
        match value {
            model::ClientKind::Cli => Self::Cli,
            model::ClientKind::Desktop => Self::Desktop,
        }
    }
}
impl From<model::ClientKind> for i32 {
    fn from(value: model::ClientKind) -> Self {
        v1::ClientKind::from(value) as i32
    }
}
impl TryFrom<i32> for model::AgentEventKind {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match v1::AgentEventKind::try_from(value) {
            Ok(v1::AgentEventKind::Locked) => Ok(Self::Locked),
            Ok(v1::AgentEventKind::Unlocked) => Ok(Self::Unlocked),
            Ok(v1::AgentEventKind::VaultChanged) => Ok(Self::VaultChanged),
            Ok(v1::AgentEventKind::ProfilesChanged) => Ok(Self::ProfilesChanged),
            Ok(v1::AgentEventKind::EntriesChanged) => Ok(Self::EntriesChanged),
            Ok(v1::AgentEventKind::BrowserChanged) => Ok(Self::BrowserChanged),
            Ok(v1::AgentEventKind::SyncChanged) => Ok(Self::SyncChanged),
            Ok(v1::AgentEventKind::LockWarning) => Ok(Self::LockWarning),
            _ => Err(invalid("AgentEventKind")),
        }
    }
}
impl From<model::AgentEventKind> for v1::AgentEventKind {
    fn from(value: model::AgentEventKind) -> Self {
        match value {
            model::AgentEventKind::Locked => Self::Locked,
            model::AgentEventKind::Unlocked => Self::Unlocked,
            model::AgentEventKind::VaultChanged => Self::VaultChanged,
            model::AgentEventKind::ProfilesChanged => Self::ProfilesChanged,
            model::AgentEventKind::EntriesChanged => Self::EntriesChanged,
            model::AgentEventKind::BrowserChanged => Self::BrowserChanged,
            model::AgentEventKind::SyncChanged => Self::SyncChanged,
            model::AgentEventKind::LockWarning => Self::LockWarning,
        }
    }
}
impl From<model::AgentEventKind> for i32 {
    fn from(value: model::AgentEventKind) -> Self {
        v1::AgentEventKind::from(value) as i32
    }
}
impl TryFrom<i32> for model::RecipientKind {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match v1::RecipientKind::try_from(value) {
            Ok(v1::RecipientKind::Passphrase) => Ok(Self::Passphrase),
            Ok(v1::RecipientKind::Age) => Ok(Self::Age),
            Ok(v1::RecipientKind::Ssh) => Ok(Self::Ssh),
            Ok(v1::RecipientKind::Plugin) => Ok(Self::Plugin),
            Ok(v1::RecipientKind::Recovery) => Ok(Self::Recovery),
            _ => Err(invalid("RecipientKind")),
        }
    }
}
impl From<model::RecipientKind> for v1::RecipientKind {
    fn from(value: model::RecipientKind) -> Self {
        match value {
            model::RecipientKind::Passphrase => Self::Passphrase,
            model::RecipientKind::Age => Self::Age,
            model::RecipientKind::Ssh => Self::Ssh,
            model::RecipientKind::Plugin => Self::Plugin,
            model::RecipientKind::Recovery => Self::Recovery,
        }
    }
}
impl From<model::RecipientKind> for i32 {
    fn from(value: model::RecipientKind) -> Self {
        v1::RecipientKind::from(value) as i32
    }
}
impl TryFrom<i32> for model::BrowserEngine {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match v1::BrowserEngine::try_from(value) {
            Ok(v1::BrowserEngine::Firefox) => Ok(Self::Firefox),
            _ => Err(invalid("BrowserEngine")),
        }
    }
}
impl From<model::BrowserEngine> for v1::BrowserEngine {
    fn from(value: model::BrowserEngine) -> Self {
        match value {
            model::BrowserEngine::Firefox => Self::Firefox,
        }
    }
}
impl From<model::BrowserEngine> for i32 {
    fn from(value: model::BrowserEngine) -> Self {
        v1::BrowserEngine::from(value) as i32
    }
}
impl TryFrom<i32> for model::AutofillScope {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match v1::AutofillScope::try_from(value) {
            Ok(v1::AutofillScope::All) => Ok(Self::All),
            Ok(v1::AutofillScope::Selected) => Ok(Self::Selected),
            Ok(v1::AutofillScope::None) => Ok(Self::None),
            _ => Err(invalid("AutofillScope")),
        }
    }
}
impl From<model::AutofillScope> for v1::AutofillScope {
    fn from(value: model::AutofillScope) -> Self {
        match value {
            model::AutofillScope::All => Self::All,
            model::AutofillScope::Selected => Self::Selected,
            model::AutofillScope::None => Self::None,
        }
    }
}
impl From<model::AutofillScope> for i32 {
    fn from(value: model::AutofillScope) -> Self {
        v1::AutofillScope::from(value) as i32
    }
}
impl TryFrom<i32> for model::EntryType {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match v1::EntryType::try_from(value) {
            Ok(v1::EntryType::Login) => Ok(Self::Login),
            Ok(v1::EntryType::Secret) => Ok(Self::Secret),
            Ok(v1::EntryType::Env) => Ok(Self::Env),
            Ok(v1::EntryType::File) => Ok(Self::File),
            Ok(v1::EntryType::Note) => Ok(Self::Note),
            Ok(v1::EntryType::SshKey) => Ok(Self::SshKey),
            Ok(v1::EntryType::Database) => Ok(Self::Database),
            Ok(v1::EntryType::ApiKey) => Ok(Self::ApiKey),
            _ => Err(invalid("EntryType")),
        }
    }
}
impl From<model::EntryType> for v1::EntryType {
    fn from(value: model::EntryType) -> Self {
        match value {
            model::EntryType::Login => Self::Login,
            model::EntryType::Secret => Self::Secret,
            model::EntryType::Env => Self::Env,
            model::EntryType::File => Self::File,
            model::EntryType::Note => Self::Note,
            model::EntryType::SshKey => Self::SshKey,
            model::EntryType::Database => Self::Database,
            model::EntryType::ApiKey => Self::ApiKey,
        }
    }
}
impl From<model::EntryType> for i32 {
    fn from(value: model::EntryType) -> Self {
        v1::EntryType::from(value) as i32
    }
}
impl TryFrom<i32> for model::TotpAlgorithm {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match v1::TotpAlgorithm::try_from(value) {
            Ok(v1::TotpAlgorithm::Sha1) => Ok(Self::Sha1),
            Ok(v1::TotpAlgorithm::Sha256) => Ok(Self::Sha256),
            Ok(v1::TotpAlgorithm::Sha512) => Ok(Self::Sha512),
            _ => Err(invalid("TotpAlgorithm")),
        }
    }
}
impl From<model::TotpAlgorithm> for v1::TotpAlgorithm {
    fn from(value: model::TotpAlgorithm) -> Self {
        match value {
            model::TotpAlgorithm::Sha1 => Self::Sha1,
            model::TotpAlgorithm::Sha256 => Self::Sha256,
            model::TotpAlgorithm::Sha512 => Self::Sha512,
        }
    }
}
impl From<model::TotpAlgorithm> for i32 {
    fn from(value: model::TotpAlgorithm) -> Self {
        v1::TotpAlgorithm::from(value) as i32
    }
}
impl TryFrom<i32> for model::EnvFormat {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match v1::EnvFormat::try_from(value) {
            Ok(v1::EnvFormat::Dotenv) => Ok(Self::Dotenv),
            Ok(v1::EnvFormat::Export) => Ok(Self::Export),
            Ok(v1::EnvFormat::Json) => Ok(Self::Json),
            _ => Err(invalid("EnvFormat")),
        }
    }
}
impl From<model::EnvFormat> for v1::EnvFormat {
    fn from(value: model::EnvFormat) -> Self {
        match value {
            model::EnvFormat::Dotenv => Self::Dotenv,
            model::EnvFormat::Export => Self::Export,
            model::EnvFormat::Json => Self::Json,
        }
    }
}
impl From<model::EnvFormat> for i32 {
    fn from(value: model::EnvFormat) -> Self {
        v1::EnvFormat::from(value) as i32
    }
}
impl TryFrom<i32> for model::VaultTrustState {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match v1::VaultTrustState::try_from(value) {
            Ok(v1::VaultTrustState::FirstUse) => Ok(Self::FirstUse),
            Ok(v1::VaultTrustState::Pinned) => Ok(Self::Pinned),
            Ok(v1::VaultTrustState::Stale) => Ok(Self::Stale),
            _ => Err(invalid("VaultTrustState")),
        }
    }
}
impl From<model::VaultTrustState> for v1::VaultTrustState {
    fn from(value: model::VaultTrustState) -> Self {
        match value {
            model::VaultTrustState::FirstUse => Self::FirstUse,
            model::VaultTrustState::Pinned => Self::Pinned,
            model::VaultTrustState::Stale => Self::Stale,
        }
    }
}
impl From<model::VaultTrustState> for i32 {
    fn from(value: model::VaultTrustState) -> Self {
        v1::VaultTrustState::from(value) as i32
    }
}
impl TryFrom<i32> for model::SyncMode {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match v1::SyncMode::try_from(value) {
            Ok(v1::SyncMode::Both) => Ok(Self::Both),
            Ok(v1::SyncMode::PullOnly) => Ok(Self::PullOnly),
            Ok(v1::SyncMode::PushOnly) => Ok(Self::PushOnly),
            _ => Err(invalid("SyncMode")),
        }
    }
}
impl From<model::SyncMode> for v1::SyncMode {
    fn from(value: model::SyncMode) -> Self {
        match value {
            model::SyncMode::Both => Self::Both,
            model::SyncMode::PullOnly => Self::PullOnly,
            model::SyncMode::PushOnly => Self::PushOnly,
        }
    }
}
impl From<model::SyncMode> for i32 {
    fn from(value: model::SyncMode) -> Self {
        v1::SyncMode::from(value) as i32
    }
}
impl TryFrom<i32> for model::Resolution {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match v1::Resolution::try_from(value) {
            Ok(v1::Resolution::Mine) => Ok(Self::Mine),
            Ok(v1::Resolution::Theirs) => Ok(Self::Theirs),
            Ok(v1::Resolution::Merged) => Ok(Self::Merged),
            _ => Err(invalid("Resolution")),
        }
    }
}
impl From<model::Resolution> for v1::Resolution {
    fn from(value: model::Resolution) -> Self {
        match value {
            model::Resolution::Mine => Self::Mine,
            model::Resolution::Theirs => Self::Theirs,
            model::Resolution::Merged => Self::Merged,
        }
    }
}
impl From<model::Resolution> for i32 {
    fn from(value: model::Resolution) -> Self {
        v1::Resolution::from(value) as i32
    }
}
impl TryFrom<i32> for model::ImportFormat {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match v1::ImportFormat::try_from(value) {
            Ok(v1::ImportFormat::Dotenv) => Ok(Self::Dotenv),
            Ok(v1::ImportFormat::Bitwarden) => Ok(Self::Bitwarden),
            Ok(v1::ImportFormat::OnePasswordCsv) => Ok(Self::OnePasswordCsv),
            Ok(v1::ImportFormat::KeepassXml) => Ok(Self::KeepassXml),
            Ok(v1::ImportFormat::BrowserCsv) => Ok(Self::BrowserCsv),
            Ok(v1::ImportFormat::Pass) => Ok(Self::Pass),
            Ok(v1::ImportFormat::Json) => Ok(Self::Json),
            Ok(v1::ImportFormat::Yaml) => Ok(Self::Yaml),
            Ok(v1::ImportFormat::LatchArchive) => Ok(Self::LatchArchive),
            _ => Err(invalid("ImportFormat")),
        }
    }
}
impl From<model::ImportFormat> for v1::ImportFormat {
    fn from(value: model::ImportFormat) -> Self {
        match value {
            model::ImportFormat::Dotenv => Self::Dotenv,
            model::ImportFormat::Bitwarden => Self::Bitwarden,
            model::ImportFormat::OnePasswordCsv => Self::OnePasswordCsv,
            model::ImportFormat::KeepassXml => Self::KeepassXml,
            model::ImportFormat::BrowserCsv => Self::BrowserCsv,
            model::ImportFormat::Pass => Self::Pass,
            model::ImportFormat::Json => Self::Json,
            model::ImportFormat::Yaml => Self::Yaml,
            model::ImportFormat::LatchArchive => Self::LatchArchive,
        }
    }
}
impl From<model::ImportFormat> for i32 {
    fn from(value: model::ImportFormat) -> Self {
        v1::ImportFormat::from(value) as i32
    }
}
impl TryFrom<i32> for model::ExportFormat {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match v1::ExportFormat::try_from(value) {
            Ok(v1::ExportFormat::LatchArchive) => Ok(Self::LatchArchive),
            Ok(v1::ExportFormat::Json) => Ok(Self::Json),
            Ok(v1::ExportFormat::Dotenv) => Ok(Self::Dotenv),
            _ => Err(invalid("ExportFormat")),
        }
    }
}
impl From<model::ExportFormat> for v1::ExportFormat {
    fn from(value: model::ExportFormat) -> Self {
        match value {
            model::ExportFormat::LatchArchive => Self::LatchArchive,
            model::ExportFormat::Json => Self::Json,
            model::ExportFormat::Dotenv => Self::Dotenv,
        }
    }
}
impl From<model::ExportFormat> for i32 {
    fn from(value: model::ExportFormat) -> Self {
        v1::ExportFormat::from(value) as i32
    }
}
impl TryFrom<i32> for model::Severity {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match v1::Severity::try_from(value) {
            Ok(v1::Severity::Info) => Ok(Self::Info),
            Ok(v1::Severity::Warning) => Ok(Self::Warning),
            Ok(v1::Severity::Error) => Ok(Self::Error),
            _ => Err(invalid("Severity")),
        }
    }
}
impl From<model::Severity> for v1::Severity {
    fn from(value: model::Severity) -> Self {
        match value {
            model::Severity::Info => Self::Info,
            model::Severity::Warning => Self::Warning,
            model::Severity::Error => Self::Error,
        }
    }
}
impl From<model::Severity> for i32 {
    fn from(value: model::Severity) -> Self {
        v1::Severity::from(value) as i32
    }
}
impl TryFrom<i32> for model::Theme {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        match v1::Theme::try_from(value) {
            Ok(v1::Theme::System) => Ok(Self::System),
            Ok(v1::Theme::Light) => Ok(Self::Light),
            Ok(v1::Theme::Dark) => Ok(Self::Dark),
            _ => Err(invalid("Theme")),
        }
    }
}
impl From<model::Theme> for v1::Theme {
    fn from(value: model::Theme) -> Self {
        match value {
            model::Theme::System => Self::System,
            model::Theme::Light => Self::Light,
            model::Theme::Dark => Self::Dark,
        }
    }
}
impl From<model::Theme> for i32 {
    fn from(value: model::Theme) -> Self {
        v1::Theme::from(value) as i32
    }
}
impl TryFrom<v1::HookParams> for model::HookParams {
    type Error = Error;
    fn try_from(value: v1::HookParams) -> Result<Self> {
        Ok(Self {
            path: value.path,
            executable: value.executable,
        })
    }
}
impl From<model::HookParams> for v1::HookParams {
    fn from(value: model::HookParams) -> Self {
        Self {
            path: value.path,
            executable: value.executable,
        }
    }
}
impl TryFrom<v1::HookResult> for model::HookResult {
    type Error = Error;
    fn try_from(value: v1::HookResult) -> Result<Self> {
        Ok(Self {
            path: value.path,
            removed: value.removed,
        })
    }
}
impl From<model::HookResult> for v1::HookResult {
    fn from(value: model::HookResult) -> Self {
        Self {
            path: value.path,
            removed: value.removed,
        }
    }
}
impl TryFrom<v1::DomainEntryTypeMetadata> for model::DomainEntryTypeMetadata {
    type Error = Error;
    fn try_from(value: v1::DomainEntryTypeMetadata) -> Result<Self> {
        Ok(Self {
            r#type: model::EntryType::try_from(value.r#type)?,
            label: value.label,
            primary_copy_field: value.primary_copy_field,
            default_payload: value
                .default_payload
                .ok_or_else(|| invalid("DomainEntryTypeMetadata.default_payload"))?
                .try_into()?,
        })
    }
}
impl From<model::DomainEntryTypeMetadata> for v1::DomainEntryTypeMetadata {
    fn from(value: model::DomainEntryTypeMetadata) -> Self {
        Self {
            r#type: value.r#type.into(),
            label: value.label,
            primary_copy_field: value.primary_copy_field,
            default_payload: Some(value.default_payload.into()),
        }
    }
}
impl TryFrom<v1::DomainCatalog> for model::DomainCatalog {
    type Error = Error;
    fn try_from(value: v1::DomainCatalog) -> Result<Self> {
        Ok(Self {
            entry_types: value
                .entry_types
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_>>()?,
            default_browser_settings: value
                .default_browser_settings
                .ok_or_else(|| invalid("DomainCatalog.default_browser_settings"))?
                .try_into()?,
            default_settings: value
                .default_settings
                .ok_or_else(|| invalid("DomainCatalog.default_settings"))?
                .try_into()?,
            default_totp: value
                .default_totp
                .ok_or_else(|| invalid("DomainCatalog.default_totp"))?
                .try_into()?,
        })
    }
}
impl From<model::DomainCatalog> for v1::DomainCatalog {
    fn from(value: model::DomainCatalog) -> Self {
        Self {
            entry_types: value.entry_types.into_iter().map(Into::into).collect(),
            default_browser_settings: Some(value.default_browser_settings.into()),
            default_settings: Some(value.default_settings.into()),
            default_totp: Some(value.default_totp.into()),
        }
    }
}
impl TryFrom<v1::ParsedOtpauth> for model::ParsedOtpauth {
    type Error = Error;
    fn try_from(value: v1::ParsedOtpauth) -> Result<Self> {
        Ok(Self {
            totp: value
                .totp
                .ok_or_else(|| invalid("ParsedOtpauth.totp"))?
                .try_into()?,
            account: value.account,
        })
    }
}
impl From<model::ParsedOtpauth> for v1::ParsedOtpauth {
    fn from(value: model::ParsedOtpauth) -> Self {
        Self {
            totp: Some(value.totp.into()),
            account: value.account,
        }
    }
}
impl TryFrom<v1::PasswordStrength> for model::PasswordStrength {
    type Error = Error;
    fn try_from(value: v1::PasswordStrength) -> Result<Self> {
        Ok(Self {
            score: value
                .score
                .try_into()
                .map_err(|_| invalid("PasswordStrength.score"))?,
            label: value.label,
            bits: value.bits,
            hints: value.hints,
        })
    }
}
impl From<model::PasswordStrength> for v1::PasswordStrength {
    fn from(value: model::PasswordStrength) -> Self {
        Self {
            score: value.score.into(),
            label: value.label,
            bits: value.bits,
            hints: value.hints,
        }
    }
}
impl TryFrom<v1::EntryDefaultsInput> for model::EntryDefaultsInput {
    type Error = Error;
    fn try_from(value: v1::EntryDefaultsInput) -> Result<Self> {
        Ok(Self {
            profile_id: value.profile_id,
            entry_type: model::EntryType::try_from(value.entry_type)?,
        })
    }
}
impl From<model::EntryDefaultsInput> for v1::EntryDefaultsInput {
    fn from(value: model::EntryDefaultsInput) -> Self {
        Self {
            profile_id: value.profile_id,
            entry_type: value.entry_type.into(),
        }
    }
}
impl TryFrom<v1::ReadFieldInput> for model::ReadFieldInput {
    type Error = Error;
    fn try_from(value: v1::ReadFieldInput) -> Result<Self> {
        Ok(Self {
            payload: value
                .payload
                .ok_or_else(|| invalid("ReadFieldInput.payload"))?
                .try_into()?,
            path: value.path,
        })
    }
}
impl From<model::ReadFieldInput> for v1::ReadFieldInput {
    fn from(value: model::ReadFieldInput) -> Self {
        Self {
            payload: Some(value.payload.into()),
            path: value.path,
        }
    }
}
impl TryFrom<v1::RedactedPayload> for model::RedactedPayload {
    type Error = Error;
    fn try_from(value: v1::RedactedPayload) -> Result<Self> {
        Ok(Self {
            payload: value
                .payload
                .ok_or_else(|| invalid("RedactedPayload.payload"))?
                .try_into()?,
            redacted: value.redacted,
        })
    }
}
impl From<model::RedactedPayload> for v1::RedactedPayload {
    fn from(value: model::RedactedPayload) -> Self {
        Self {
            payload: Some(value.payload.into()),
            redacted: value.redacted,
        }
    }
}
impl TryFrom<v1::PayloadMergeInput> for model::PayloadMergeInput {
    type Error = Error;
    fn try_from(value: v1::PayloadMergeInput) -> Result<Self> {
        Ok(Self {
            stored: value
                .stored
                .ok_or_else(|| invalid("PayloadMergeInput.stored"))?
                .try_into()?,
            input: value
                .input
                .ok_or_else(|| invalid("PayloadMergeInput.input"))?
                .try_into()?,
        })
    }
}
impl From<model::PayloadMergeInput> for v1::PayloadMergeInput {
    fn from(value: model::PayloadMergeInput) -> Self {
        Self {
            stored: Some(value.stored.into()),
            input: Some(value.input.into()),
        }
    }
}
impl TryFrom<v1::MergeEntriesInput> for model::MergeEntriesInput {
    type Error = Error;
    fn try_from(value: v1::MergeEntriesInput) -> Result<Self> {
        Ok(Self {
            mine: value
                .mine
                .ok_or_else(|| invalid("MergeEntriesInput.mine"))?
                .try_into()?,
            theirs: value
                .theirs
                .ok_or_else(|| invalid("MergeEntriesInput.theirs"))?
                .try_into()?,
            from_theirs: value.from_theirs,
        })
    }
}
impl From<model::MergeEntriesInput> for v1::MergeEntriesInput {
    fn from(value: model::MergeEntriesInput) -> Self {
        Self {
            mine: Some(value.mine.into()),
            theirs: Some(value.theirs.into()),
            from_theirs: value.from_theirs,
        }
    }
}
impl TryFrom<v1::DomainEnvFormatInput> for model::DomainEnvFormatInput {
    type Error = Error;
    fn try_from(value: v1::DomainEnvFormatInput) -> Result<Self> {
        Ok(Self {
            vars: value
                .vars
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_>>()?,
            format: model::EnvFormat::try_from(value.format)?,
        })
    }
}
impl From<model::DomainEnvFormatInput> for v1::DomainEnvFormatInput {
    fn from(value: model::DomainEnvFormatInput) -> Self {
        Self {
            vars: value.vars.into_iter().map(Into::into).collect(),
            format: value.format.into(),
        }
    }
}
impl TryFrom<v1::ValidateRecipientInput> for model::ValidateRecipientInput {
    type Error = Error;
    fn try_from(value: v1::ValidateRecipientInput) -> Result<Self> {
        Ok(Self {
            label: value.label,
            key: value.key,
            existing: value
                .existing
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_>>()?,
        })
    }
}
impl From<model::ValidateRecipientInput> for v1::ValidateRecipientInput {
    fn from(value: model::ValidateRecipientInput) -> Self {
        Self {
            label: value.label,
            key: value.key,
            existing: value.existing.into_iter().map(Into::into).collect(),
        }
    }
}
impl TryFrom<v1::DiffRowsInput> for model::DiffRowsInput {
    type Error = Error;
    fn try_from(value: v1::DiffRowsInput) -> Result<Self> {
        Ok(Self {
            mine: value.mine.map(TryInto::try_into).transpose()?,
            theirs: value.theirs.map(TryInto::try_into).transpose()?,
        })
    }
}
impl From<model::DiffRowsInput> for v1::DiffRowsInput {
    fn from(value: model::DiffRowsInput) -> Self {
        Self {
            mine: value.mine.map(Into::into),
            theirs: value.theirs.map(Into::into),
        }
    }
}
impl TryFrom<v1::DomainDiffRow> for model::DomainDiffRow {
    type Error = Error;
    fn try_from(value: v1::DomainDiffRow) -> Result<Self> {
        Ok(Self {
            field: value.field,
            mine: value.mine,
            theirs: value.theirs,
            differs: value.differs,
            content_differs: value.content_differs,
        })
    }
}
impl From<model::DomainDiffRow> for v1::DomainDiffRow {
    fn from(value: model::DomainDiffRow) -> Self {
        Self {
            field: value.field,
            mine: value.mine,
            theirs: value.theirs,
            differs: value.differs,
            content_differs: value.content_differs,
        }
    }
}
impl TryFrom<v1::HelloParams> for model::HelloParams {
    type Error = Error;
    fn try_from(value: v1::HelloParams) -> Result<Self> {
        Ok(Self {
            client: model::ClientKind::try_from(value.client)?,
            version: value.version,
        })
    }
}
impl From<model::HelloParams> for v1::HelloParams {
    fn from(value: model::HelloParams) -> Self {
        Self {
            client: value.client.into(),
            version: value.version,
        }
    }
}
impl TryFrom<v1::HelloResult> for model::HelloResult {
    type Error = Error;
    fn try_from(value: v1::HelloResult) -> Result<Self> {
        Ok(Self {
            agent_version: value.agent_version,
            protocol: value.protocol,
        })
    }
}
impl From<model::HelloResult> for v1::HelloResult {
    fn from(value: model::HelloResult) -> Self {
        Self {
            agent_version: value.agent_version,
            protocol: value.protocol,
        }
    }
}
impl TryFrom<v1::AgentEvent> for model::AgentEvent {
    type Error = Error;
    fn try_from(value: v1::AgentEvent) -> Result<Self> {
        Ok(Self {
            kind: model::AgentEventKind::try_from(value.kind)?,
            profile_id: value.profile_id,
            entry_id: value.entry_id,
            message: value.message,
        })
    }
}
impl From<model::AgentEvent> for v1::AgentEvent {
    fn from(value: model::AgentEvent) -> Self {
        Self {
            kind: value.kind.into(),
            profile_id: value.profile_id,
            entry_id: value.entry_id,
            message: value.message,
        }
    }
}
impl TryFrom<v1::VaultStatus> for model::VaultStatus {
    type Error = Error;
    fn try_from(value: v1::VaultStatus) -> Result<Self> {
        Ok(Self {
            initialized: value.initialized,
            locked: value.locked,
            vault_id: value.vault_id,
            path: value.path,
            recipients: value
                .recipients
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_>>()?,
            dirty: value.dirty,
            ahead: value.ahead,
            behind: value.behind,
            has_remote: value.has_remote,
            last_sync_at: value.last_sync_at,
            locks_in_secs: value.locks_in_secs,
            trust: value.trust.map(TryInto::try_into).transpose()?,
            integrity_problems: value.integrity_problems,
            rotation_pending: value.rotation_pending,
            memory_locked: value.memory_locked,
        })
    }
}
impl From<model::VaultStatus> for v1::VaultStatus {
    fn from(value: model::VaultStatus) -> Self {
        Self {
            initialized: value.initialized,
            locked: value.locked,
            vault_id: value.vault_id,
            path: value.path,
            recipients: value.recipients.into_iter().map(Into::into).collect(),
            dirty: value.dirty,
            ahead: value.ahead,
            behind: value.behind,
            has_remote: value.has_remote,
            last_sync_at: value.last_sync_at,
            locks_in_secs: value.locks_in_secs,
            trust: value.trust.map(Into::into),
            integrity_problems: value.integrity_problems,
            rotation_pending: value.rotation_pending,
            memory_locked: value.memory_locked,
        }
    }
}
impl TryFrom<v1::TrustStatus> for model::TrustStatus {
    type Error = Error;
    fn try_from(value: v1::TrustStatus) -> Result<Self> {
        Ok(Self {
            state: model::VaultTrustState::try_from(value.state)?,
            fingerprint: value.fingerprint,
            key_seq: value.key_seq,
        })
    }
}
impl From<model::TrustStatus> for v1::TrustStatus {
    fn from(value: model::TrustStatus) -> Self {
        Self {
            state: value.state.into(),
            fingerprint: value.fingerprint,
            key_seq: value.key_seq,
        }
    }
}
impl TryFrom<v1::ForgetTrustParams> for model::ForgetTrustParams {
    type Error = Error;
    fn try_from(value: v1::ForgetTrustParams) -> Result<Self> {
        Ok(Self {
            confirm: value.confirm,
        })
    }
}
impl From<model::ForgetTrustParams> for v1::ForgetTrustParams {
    fn from(value: model::ForgetTrustParams) -> Self {
        Self {
            confirm: value.confirm,
        }
    }
}
impl TryFrom<v1::RecipientInfo> for model::RecipientInfo {
    type Error = Error;
    fn try_from(value: v1::RecipientInfo) -> Result<Self> {
        Ok(Self {
            label: value.label,
            kind: model::RecipientKind::try_from(value.kind)?,
            public_key: value.public_key,
            added_at: value.added_at,
        })
    }
}
impl From<model::RecipientInfo> for v1::RecipientInfo {
    fn from(value: model::RecipientInfo) -> Self {
        Self {
            label: value.label,
            kind: value.kind.into(),
            public_key: value.public_key,
            added_at: value.added_at,
        }
    }
}
impl TryFrom<v1::KdfParams> for model::KdfParams {
    type Error = Error;
    fn try_from(value: v1::KdfParams) -> Result<Self> {
        Ok(Self {
            memory_kib: value.memory_kib,
            iterations: value.iterations,
            parallelism: value.parallelism,
        })
    }
}
impl From<model::KdfParams> for v1::KdfParams {
    fn from(value: model::KdfParams) -> Self {
        Self {
            memory_kib: value.memory_kib,
            iterations: value.iterations,
            parallelism: value.parallelism,
        }
    }
}
impl TryFrom<v1::InitParams> for model::InitParams {
    type Error = Error;
    fn try_from(value: v1::InitParams) -> Result<Self> {
        Ok(Self {
            path: value.path,
            passphrase: value.passphrase,
            kdf: value.kdf.map(TryInto::try_into).transpose()?,
            git_name: value.git_name,
            git_email: value.git_email,
            remote_url: value.remote_url,
            recovery_kit: value.recovery_kit,
        })
    }
}
impl From<model::InitParams> for v1::InitParams {
    fn from(value: model::InitParams) -> Self {
        Self {
            path: value.path,
            passphrase: value.passphrase,
            kdf: value.kdf.map(Into::into),
            git_name: value.git_name,
            git_email: value.git_email,
            remote_url: value.remote_url,
            recovery_kit: value.recovery_kit,
        }
    }
}
impl TryFrom<v1::InitResult> for model::InitResult {
    type Error = Error;
    fn try_from(value: v1::InitResult) -> Result<Self> {
        Ok(Self {
            status: value
                .status
                .ok_or_else(|| invalid("InitResult.status"))?
                .try_into()?,
            recovery_kit: value.recovery_kit.map(TryInto::try_into).transpose()?,
        })
    }
}
impl From<model::InitResult> for v1::InitResult {
    fn from(value: model::InitResult) -> Self {
        Self {
            status: Some(value.status.into()),
            recovery_kit: value.recovery_kit.map(Into::into),
        }
    }
}
impl TryFrom<v1::RecoveryKit> for model::RecoveryKit {
    type Error = Error;
    fn try_from(value: v1::RecoveryKit) -> Result<Self> {
        Ok(Self {
            vault_id: value.vault_id,
            passphrase_public_key: value.passphrase_public_key,
            recovery_public_key: value.recovery_public_key,
            recovery_secret_key: value.recovery_secret_key,
            document: value.document,
        })
    }
}
impl From<model::RecoveryKit> for v1::RecoveryKit {
    fn from(value: model::RecoveryKit) -> Self {
        Self {
            vault_id: value.vault_id,
            passphrase_public_key: value.passphrase_public_key,
            recovery_public_key: value.recovery_public_key,
            recovery_secret_key: value.recovery_secret_key,
            document: value.document,
        }
    }
}
impl TryFrom<v1::CloneParams> for model::CloneParams {
    type Error = Error;
    fn try_from(value: v1::CloneParams) -> Result<Self> {
        Ok(Self {
            url: value.url,
            path: value.path,
        })
    }
}
impl From<model::CloneParams> for v1::CloneParams {
    fn from(value: model::CloneParams) -> Self {
        Self {
            url: value.url,
            path: value.path,
        }
    }
}
impl TryFrom<v1::OpenParams> for model::OpenParams {
    type Error = Error;
    fn try_from(value: v1::OpenParams) -> Result<Self> {
        Ok(Self { path: value.path })
    }
}
impl From<model::OpenParams> for v1::OpenParams {
    fn from(value: model::OpenParams) -> Self {
        Self { path: value.path }
    }
}
impl TryFrom<v1::KnownVault> for model::KnownVault {
    type Error = Error;
    fn try_from(value: v1::KnownVault) -> Result<Self> {
        Ok(Self {
            path: value.path,
            vault_id: value.vault_id,
            active: value.active,
        })
    }
}
impl From<model::KnownVault> for v1::KnownVault {
    fn from(value: model::KnownVault) -> Self {
        Self {
            path: value.path,
            vault_id: value.vault_id,
            active: value.active,
        }
    }
}
impl TryFrom<v1::UnlockParams> for model::UnlockParams {
    type Error = Error;
    fn try_from(value: v1::UnlockParams) -> Result<Self> {
        Ok(Self {
            passphrase: value.passphrase,
            identity_file: value.identity_file,
            timeout_secs: value.timeout_secs,
        })
    }
}
impl From<model::UnlockParams> for v1::UnlockParams {
    fn from(value: model::UnlockParams) -> Self {
        Self {
            passphrase: value.passphrase,
            identity_file: value.identity_file,
            timeout_secs: value.timeout_secs,
        }
    }
}
impl TryFrom<v1::KdfBenchmarkParams> for model::KdfBenchmarkParams {
    type Error = Error;
    fn try_from(value: v1::KdfBenchmarkParams) -> Result<Self> {
        Ok(Self {
            target_ms: value.target_ms,
        })
    }
}
impl From<model::KdfBenchmarkParams> for v1::KdfBenchmarkParams {
    fn from(value: model::KdfBenchmarkParams) -> Self {
        Self {
            target_ms: value.target_ms,
        }
    }
}
impl TryFrom<v1::KdfBenchmark> for model::KdfBenchmark {
    type Error = Error;
    fn try_from(value: v1::KdfBenchmark) -> Result<Self> {
        Ok(Self {
            params: value
                .params
                .ok_or_else(|| invalid("KdfBenchmark.params"))?
                .try_into()?,
            measured_ms: value.measured_ms,
        })
    }
}
impl From<model::KdfBenchmark> for v1::KdfBenchmark {
    fn from(value: model::KdfBenchmark) -> Self {
        Self {
            params: Some(value.params.into()),
            measured_ms: value.measured_ms,
        }
    }
}
impl TryFrom<v1::ChangePassphraseParams> for model::ChangePassphraseParams {
    type Error = Error;
    fn try_from(value: v1::ChangePassphraseParams) -> Result<Self> {
        Ok(Self {
            current: value.current,
            new_passphrase: value.new_passphrase,
        })
    }
}
impl From<model::ChangePassphraseParams> for v1::ChangePassphraseParams {
    fn from(value: model::ChangePassphraseParams) -> Self {
        Self {
            current: value.current,
            new_passphrase: value.new_passphrase,
        }
    }
}
impl TryFrom<v1::LaunchAction> for model::LaunchAction {
    type Error = Error;
    fn try_from(value: v1::LaunchAction) -> Result<Self> {
        Ok(Self {
            id: value.id,
            name: value.name,
            command: value.command,
            env_entry_ids: value.env_entry_ids,
            env_keys: value.env_keys.map(|value| value.items),
            urls: value.urls,
        })
    }
}
impl From<model::LaunchAction> for v1::LaunchAction {
    fn from(value: model::LaunchAction) -> Self {
        Self {
            id: value.id,
            name: value.name,
            command: value.command,
            env_entry_ids: value.env_entry_ids,
            env_keys: value
                .env_keys
                .map(|value| v1::OptionalStringList { items: value }),
            urls: value.urls,
        }
    }
}
impl TryFrom<v1::ActionInput> for model::ActionInput {
    type Error = Error;
    fn try_from(value: v1::ActionInput) -> Result<Self> {
        Ok(Self {
            name: value.name,
            command: value.command,
            env_entry_ids: value.env_entry_ids,
            env_keys: value.env_keys.map(|value| value.items),
            urls: value.urls,
        })
    }
}
impl From<model::ActionInput> for v1::ActionInput {
    fn from(value: model::ActionInput) -> Self {
        Self {
            name: value.name,
            command: value.command,
            env_entry_ids: value.env_entry_ids,
            env_keys: value
                .env_keys
                .map(|value| v1::OptionalStringList { items: value }),
            urls: value.urls,
        }
    }
}
impl TryFrom<v1::ActionSaveParams> for model::ActionSaveParams {
    type Error = Error;
    fn try_from(value: v1::ActionSaveParams) -> Result<Self> {
        Ok(Self {
            profile_id: value.profile_id,
            id: value.id,
            input: value
                .input
                .ok_or_else(|| invalid("ActionSaveParams.input"))?
                .try_into()?,
        })
    }
}
impl From<model::ActionSaveParams> for v1::ActionSaveParams {
    fn from(value: model::ActionSaveParams) -> Self {
        Self {
            profile_id: value.profile_id,
            id: value.id,
            input: Some(value.input.into()),
        }
    }
}
impl TryFrom<v1::ActionListResult> for model::ActionListResult {
    type Error = Error;
    fn try_from(value: v1::ActionListResult) -> Result<Self> {
        Ok(Self {
            actions: value
                .actions
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_>>()?,
            working_directory: value.working_directory,
        })
    }
}
impl From<model::ActionListResult> for v1::ActionListResult {
    fn from(value: model::ActionListResult) -> Self {
        Self {
            actions: value.actions.into_iter().map(Into::into).collect(),
            working_directory: value.working_directory,
        }
    }
}
impl TryFrom<v1::ActionConfigureParams> for model::ActionConfigureParams {
    type Error = Error;
    fn try_from(value: v1::ActionConfigureParams) -> Result<Self> {
        Ok(Self {
            profile_id: value.profile_id,
            working_directory: value.working_directory,
        })
    }
}
impl From<model::ActionConfigureParams> for v1::ActionConfigureParams {
    fn from(value: model::ActionConfigureParams) -> Self {
        Self {
            profile_id: value.profile_id,
            working_directory: value.working_directory,
        }
    }
}
impl TryFrom<v1::ActionIdParams> for model::ActionIdParams {
    type Error = Error;
    fn try_from(value: v1::ActionIdParams) -> Result<Self> {
        Ok(Self {
            profile_id: value.profile_id,
            action_id: value.action_id,
        })
    }
}
impl From<model::ActionIdParams> for v1::ActionIdParams {
    fn from(value: model::ActionIdParams) -> Self {
        Self {
            profile_id: value.profile_id,
            action_id: value.action_id,
        }
    }
}
impl TryFrom<v1::ProfileSummary> for model::ProfileSummary {
    type Error = Error;
    fn try_from(value: v1::ProfileSummary) -> Result<Self> {
        Ok(Self {
            id: value.id,
            name: value.name,
            color: value.color,
            icon: value.icon,
            description: value.description,
            pinned: value.pinned,
            entry_count: value.entry_count,
            browser_running: value.browser_running,
        })
    }
}
impl From<model::ProfileSummary> for v1::ProfileSummary {
    fn from(value: model::ProfileSummary) -> Self {
        Self {
            id: value.id,
            name: value.name,
            color: value.color,
            icon: value.icon,
            description: value.description,
            pinned: value.pinned,
            entry_count: value.entry_count,
            browser_running: value.browser_running,
        }
    }
}
impl TryFrom<v1::Profile> for model::Profile {
    type Error = Error;
    fn try_from(value: v1::Profile) -> Result<Self> {
        Ok(Self {
            id: value.id,
            name: value.name,
            color: value.color,
            icon: value.icon,
            description: value.description,
            pinned: value.pinned,
            browser: value
                .browser
                .ok_or_else(|| invalid("Profile.browser"))?
                .try_into()?,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}
impl From<model::Profile> for v1::Profile {
    fn from(value: model::Profile) -> Self {
        Self {
            id: value.id,
            name: value.name,
            color: value.color,
            icon: value.icon,
            description: value.description,
            pinned: value.pinned,
            browser: Some(value.browser.into()),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}
impl TryFrom<v1::BrowserSettings> for model::BrowserSettings {
    type Error = Error;
    fn try_from(value: v1::BrowserSettings) -> Result<Self> {
        Ok(Self {
            start_pages: value.start_pages,
            bookmarks: value
                .bookmarks
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_>>()?,
            autofill: model::AutofillScope::try_from(value.autofill)?,
            autofill_entry_ids: value.autofill_entry_ids,
        })
    }
}
impl From<model::BrowserSettings> for v1::BrowserSettings {
    fn from(value: model::BrowserSettings) -> Self {
        Self {
            start_pages: value.start_pages,
            bookmarks: value.bookmarks.into_iter().map(Into::into).collect(),
            autofill: value.autofill.into(),
            autofill_entry_ids: value.autofill_entry_ids,
        }
    }
}
impl TryFrom<v1::Bookmark> for model::Bookmark {
    type Error = Error;
    fn try_from(value: v1::Bookmark) -> Result<Self> {
        Ok(Self {
            title: value.title,
            url: value.url,
        })
    }
}
impl From<model::Bookmark> for v1::Bookmark {
    fn from(value: model::Bookmark) -> Self {
        Self {
            title: value.title,
            url: value.url,
        }
    }
}
impl TryFrom<v1::ProfileInput> for model::ProfileInput {
    type Error = Error;
    fn try_from(value: v1::ProfileInput) -> Result<Self> {
        Ok(Self {
            name: value.name,
            color: value.color,
            icon: value.icon,
            description: value.description,
            pinned: value.pinned,
            browser: value
                .browser
                .ok_or_else(|| invalid("ProfileInput.browser"))?
                .try_into()?,
        })
    }
}
impl From<model::ProfileInput> for v1::ProfileInput {
    fn from(value: model::ProfileInput) -> Self {
        Self {
            name: value.name,
            color: value.color,
            icon: value.icon,
            description: value.description,
            pinned: value.pinned,
            browser: Some(value.browser.into()),
        }
    }
}
impl TryFrom<v1::ProfileUpdateParams> for model::ProfileUpdateParams {
    type Error = Error;
    fn try_from(value: v1::ProfileUpdateParams) -> Result<Self> {
        Ok(Self {
            id: value.id,
            input: value
                .input
                .ok_or_else(|| invalid("ProfileUpdateParams.input"))?
                .try_into()?,
        })
    }
}
impl From<model::ProfileUpdateParams> for v1::ProfileUpdateParams {
    fn from(value: model::ProfileUpdateParams) -> Self {
        Self {
            id: value.id,
            input: Some(value.input.into()),
        }
    }
}
impl TryFrom<v1::IdParams> for model::IdParams {
    type Error = Error;
    fn try_from(value: v1::IdParams) -> Result<Self> {
        Ok(Self { id: value.id })
    }
}
impl From<model::IdParams> for v1::IdParams {
    fn from(value: model::IdParams) -> Self {
        Self { id: value.id }
    }
}
impl TryFrom<v1::EntrySummary> for model::EntrySummary {
    type Error = Error;
    fn try_from(value: v1::EntrySummary) -> Result<Self> {
        Ok(Self {
            id: value.id,
            profile_id: value.profile_id,
            entry_type: model::EntryType::try_from(value.r#type)?,
            name: value.name,
            tags: value.tags,
            favorite: value.favorite,
            subtitle: value.subtitle,
            has_totp: value.has_totp,
            updated_at: value.updated_at,
        })
    }
}
impl From<model::EntrySummary> for v1::EntrySummary {
    fn from(value: model::EntrySummary) -> Self {
        Self {
            id: value.id,
            profile_id: value.profile_id,
            r#type: value.entry_type.into(),
            name: value.name,
            tags: value.tags,
            favorite: value.favorite,
            subtitle: value.subtitle,
            has_totp: value.has_totp,
            updated_at: value.updated_at,
        }
    }
}
impl TryFrom<v1::Entry> for model::Entry {
    type Error = Error;
    fn try_from(value: v1::Entry) -> Result<Self> {
        Ok(Self {
            id: value.id,
            profile_id: value.profile_id,
            name: value.name,
            tags: value.tags,
            notes: value.notes,
            favorite: value.favorite,
            created_at: value.created_at,
            updated_at: value.updated_at,
            payload: value
                .payload
                .ok_or_else(|| invalid("Entry.payload"))?
                .try_into()?,
            redacted_fields: value.redacted_fields,
        })
    }
}
impl From<model::Entry> for v1::Entry {
    fn from(value: model::Entry) -> Self {
        Self {
            id: value.id,
            profile_id: value.profile_id,
            name: value.name,
            tags: value.tags,
            notes: value.notes,
            favorite: value.favorite,
            created_at: value.created_at,
            updated_at: value.updated_at,
            payload: Some(value.payload.into()),
            redacted_fields: value.redacted_fields,
        }
    }
}
impl TryFrom<v1::EntryInput> for model::EntryInput {
    type Error = Error;
    fn try_from(value: v1::EntryInput) -> Result<Self> {
        Ok(Self {
            profile_id: value.profile_id,
            name: value.name,
            tags: value.tags,
            notes: value.notes,
            favorite: value.favorite,
            payload: value
                .payload
                .ok_or_else(|| invalid("EntryInput.payload"))?
                .try_into()?,
        })
    }
}
impl From<model::EntryInput> for v1::EntryInput {
    fn from(value: model::EntryInput) -> Self {
        Self {
            profile_id: value.profile_id,
            name: value.name,
            tags: value.tags,
            notes: value.notes,
            favorite: value.favorite,
            payload: Some(value.payload.into()),
        }
    }
}
impl TryFrom<v1::EntryUpdateParams> for model::EntryUpdateParams {
    type Error = Error;
    fn try_from(value: v1::EntryUpdateParams) -> Result<Self> {
        Ok(Self {
            id: value.id,
            input: value
                .input
                .ok_or_else(|| invalid("EntryUpdateParams.input"))?
                .try_into()?,
        })
    }
}
impl From<model::EntryUpdateParams> for v1::EntryUpdateParams {
    fn from(value: model::EntryUpdateParams) -> Self {
        Self {
            id: value.id,
            input: Some(value.input.into()),
        }
    }
}
impl TryFrom<v1::ListEntriesParams> for model::ListEntriesParams {
    type Error = Error;
    fn try_from(value: v1::ListEntriesParams) -> Result<Self> {
        Ok(Self {
            profile_id: value.profile_id,
            query: value.query,
            entry_type: value.r#type.map(model::EntryType::try_from).transpose()?,
        })
    }
}
impl From<model::ListEntriesParams> for v1::ListEntriesParams {
    fn from(value: model::ListEntriesParams) -> Self {
        Self {
            profile_id: value.profile_id,
            query: value.query,
            r#type: value.entry_type.map(Into::into),
        }
    }
}
impl TryFrom<v1::GetEntryParams> for model::GetEntryParams {
    type Error = Error;
    fn try_from(value: v1::GetEntryParams) -> Result<Self> {
        Ok(Self {
            id: value.id,
            reveal: value.reveal,
        })
    }
}
impl From<model::GetEntryParams> for v1::GetEntryParams {
    fn from(value: model::GetEntryParams) -> Self {
        Self {
            id: value.id,
            reveal: value.reveal,
        }
    }
}
impl TryFrom<v1::RevealParams> for model::RevealParams {
    type Error = Error;
    fn try_from(value: v1::RevealParams) -> Result<Self> {
        Ok(Self {
            id: value.id,
            field: value.field,
        })
    }
}
impl From<model::RevealParams> for v1::RevealParams {
    fn from(value: model::RevealParams) -> Self {
        Self {
            id: value.id,
            field: value.field,
        }
    }
}
impl TryFrom<v1::RevealResult> for model::RevealResult {
    type Error = Error;
    fn try_from(value: v1::RevealResult) -> Result<Self> {
        Ok(Self { value: value.value })
    }
}
impl From<model::RevealResult> for v1::RevealResult {
    fn from(value: model::RevealResult) -> Self {
        Self { value: value.value }
    }
}
impl TryFrom<v1::MoveEntryParams> for model::MoveEntryParams {
    type Error = Error;
    fn try_from(value: v1::MoveEntryParams) -> Result<Self> {
        Ok(Self {
            id: value.id,
            profile_id: value.profile_id,
        })
    }
}
impl From<model::MoveEntryParams> for v1::MoveEntryParams {
    fn from(value: model::MoveEntryParams) -> Self {
        Self {
            id: value.id,
            profile_id: value.profile_id,
        }
    }
}
impl TryFrom<v1::HistoryItem> for model::HistoryItem {
    type Error = Error;
    fn try_from(value: v1::HistoryItem) -> Result<Self> {
        Ok(Self {
            commit: value.commit,
            at: value.at,
            author: value.author,
            message: value.message,
            changed_fields: value.changed_fields,
            deleted: value.deleted,
        })
    }
}
impl From<model::HistoryItem> for v1::HistoryItem {
    fn from(value: model::HistoryItem) -> Self {
        Self {
            commit: value.commit,
            at: value.at,
            author: value.author,
            message: value.message,
            changed_fields: value.changed_fields,
            deleted: value.deleted,
        }
    }
}
impl TryFrom<v1::RestoreParams> for model::RestoreParams {
    type Error = Error;
    fn try_from(value: v1::RestoreParams) -> Result<Self> {
        Ok(Self {
            id: value.id,
            commit: value.commit,
        })
    }
}
impl From<model::RestoreParams> for v1::RestoreParams {
    fn from(value: model::RestoreParams) -> Self {
        Self {
            id: value.id,
            commit: value.commit,
        }
    }
}
impl TryFrom<v1::TrashParams> for model::TrashParams {
    type Error = Error;
    fn try_from(value: v1::TrashParams) -> Result<Self> {
        Ok(Self {
            profile_id: value.profile_id,
        })
    }
}
impl From<model::TrashParams> for v1::TrashParams {
    fn from(value: model::TrashParams) -> Self {
        Self {
            profile_id: value.profile_id,
        }
    }
}
impl TryFrom<v1::TrashItem> for model::TrashItem {
    type Error = Error;
    fn try_from(value: v1::TrashItem) -> Result<Self> {
        Ok(Self {
            id: value.id,
            profile_id: value.profile_id,
            name: value.name,
            entry_type: model::EntryType::try_from(value.r#type)?,
            deleted_at: value.deleted_at,
            commit: value.commit,
        })
    }
}
impl From<model::TrashItem> for v1::TrashItem {
    fn from(value: model::TrashItem) -> Self {
        Self {
            id: value.id,
            profile_id: value.profile_id,
            name: value.name,
            r#type: value.entry_type.into(),
            deleted_at: value.deleted_at,
            commit: value.commit,
        }
    }
}
impl TryFrom<v1::LoginData> for model::LoginData {
    type Error = Error;
    fn try_from(value: v1::LoginData) -> Result<Self> {
        Ok(Self {
            username: value.username,
            password: value.password,
            urls: value.urls,
            totp: value.totp.map(TryInto::try_into).transpose()?,
            custom: value
                .custom
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_>>()?,
        })
    }
}
impl From<model::LoginData> for v1::LoginData {
    fn from(value: model::LoginData) -> Self {
        Self {
            username: value.username,
            password: value.password,
            urls: value.urls,
            totp: value.totp.map(Into::into),
            custom: value.custom.into_iter().map(Into::into).collect(),
        }
    }
}
impl TryFrom<v1::TotpConfig> for model::TotpConfig {
    type Error = Error;
    fn try_from(value: v1::TotpConfig) -> Result<Self> {
        Ok(Self {
            secret: value.secret,
            algorithm: model::TotpAlgorithm::try_from(value.algorithm)?,
            digits: value
                .digits
                .try_into()
                .map_err(|_| invalid("TotpConfig.digits"))?,
            period: value.period,
            issuer: value.issuer,
        })
    }
}
impl From<model::TotpConfig> for v1::TotpConfig {
    fn from(value: model::TotpConfig) -> Self {
        Self {
            secret: value.secret,
            algorithm: value.algorithm.into(),
            digits: value.digits.into(),
            period: value.period,
            issuer: value.issuer,
        }
    }
}
impl TryFrom<v1::CustomField> for model::CustomField {
    type Error = Error;
    fn try_from(value: v1::CustomField) -> Result<Self> {
        Ok(Self {
            key: value.key,
            value: value.value,
            hidden: value.hidden,
        })
    }
}
impl From<model::CustomField> for v1::CustomField {
    fn from(value: model::CustomField) -> Self {
        Self {
            key: value.key,
            value: value.value,
            hidden: value.hidden,
        }
    }
}
impl TryFrom<v1::SecretData> for model::SecretData {
    type Error = Error;
    fn try_from(value: v1::SecretData) -> Result<Self> {
        Ok(Self { value: value.value })
    }
}
impl From<model::SecretData> for v1::SecretData {
    fn from(value: model::SecretData) -> Self {
        Self { value: value.value }
    }
}
impl TryFrom<v1::EnvData> for model::EnvData {
    type Error = Error;
    fn try_from(value: v1::EnvData) -> Result<Self> {
        Ok(Self {
            vars: value
                .vars
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_>>()?,
        })
    }
}
impl From<model::EnvData> for v1::EnvData {
    fn from(value: model::EnvData) -> Self {
        Self {
            vars: value.vars.into_iter().map(Into::into).collect(),
        }
    }
}
impl TryFrom<v1::EnvVar> for model::EnvVar {
    type Error = Error;
    fn try_from(value: v1::EnvVar) -> Result<Self> {
        Ok(Self {
            key: value.key,
            value: value.value,
            hidden: value.hidden,
            comment: value.comment,
        })
    }
}
impl From<model::EnvVar> for v1::EnvVar {
    fn from(value: model::EnvVar) -> Self {
        Self {
            key: value.key,
            value: value.value,
            hidden: value.hidden,
            comment: value.comment,
        }
    }
}
impl TryFrom<v1::FileData> for model::FileData {
    type Error = Error;
    fn try_from(value: v1::FileData) -> Result<Self> {
        Ok(Self {
            filename: value.filename,
            mime: value.mime,
            content_base64: value.content_base64,
            size: value.size,
        })
    }
}
impl From<model::FileData> for v1::FileData {
    fn from(value: model::FileData) -> Self {
        Self {
            filename: value.filename,
            mime: value.mime,
            content_base64: value.content_base64,
            size: value.size,
        }
    }
}
impl TryFrom<v1::NoteData> for model::NoteData {
    type Error = Error;
    fn try_from(value: v1::NoteData) -> Result<Self> {
        Ok(Self { body: value.body })
    }
}
impl From<model::NoteData> for v1::NoteData {
    fn from(value: model::NoteData) -> Self {
        Self { body: value.body }
    }
}
impl TryFrom<v1::SshKeyData> for model::SshKeyData {
    type Error = Error;
    fn try_from(value: v1::SshKeyData) -> Result<Self> {
        Ok(Self {
            private_key: value.private_key,
            public_key: value.public_key,
            passphrase: value.passphrase,
            comment: value.comment,
        })
    }
}
impl From<model::SshKeyData> for v1::SshKeyData {
    fn from(value: model::SshKeyData) -> Self {
        Self {
            private_key: value.private_key,
            public_key: value.public_key,
            passphrase: value.passphrase,
            comment: value.comment,
        }
    }
}
impl TryFrom<v1::DatabaseData> for model::DatabaseData {
    type Error = Error;
    fn try_from(value: v1::DatabaseData) -> Result<Self> {
        Ok(Self {
            engine: value.engine,
            host: value.host,
            port: value
                .port
                .map(|value| value.try_into().map_err(|_| invalid("DatabaseData.port")))
                .transpose()?,
            database: value.database,
            username: value.username,
            password: value.password,
            options: value.options,
        })
    }
}
impl From<model::DatabaseData> for v1::DatabaseData {
    fn from(value: model::DatabaseData) -> Self {
        Self {
            engine: value.engine,
            host: value.host,
            port: value.port.map(Into::into),
            database: value.database,
            username: value.username,
            password: value.password,
            options: value.options,
        }
    }
}
impl TryFrom<v1::ApiKeyData> for model::ApiKeyData {
    type Error = Error;
    fn try_from(value: v1::ApiKeyData) -> Result<Self> {
        Ok(Self {
            key: value.key,
            secret: value.secret,
            endpoint: value.endpoint,
            expires_at: value.expires_at,
        })
    }
}
impl From<model::ApiKeyData> for v1::ApiKeyData {
    fn from(value: model::ApiKeyData) -> Self {
        Self {
            key: value.key,
            secret: value.secret,
            endpoint: value.endpoint,
            expires_at: value.expires_at,
        }
    }
}
impl TryFrom<v1::EnvResolveParams> for model::EnvResolveParams {
    type Error = Error;
    fn try_from(value: v1::EnvResolveParams) -> Result<Self> {
        Ok(Self {
            profile_id: value.profile_id,
            entry_ids: value.entry_ids.map(|value| value.items),
        })
    }
}
impl From<model::EnvResolveParams> for v1::EnvResolveParams {
    fn from(value: model::EnvResolveParams) -> Self {
        Self {
            profile_id: value.profile_id,
            entry_ids: value
                .entry_ids
                .map(|value| v1::OptionalStringList { items: value }),
        }
    }
}
impl TryFrom<v1::EnvResolveResult> for model::EnvResolveResult {
    type Error = Error;
    fn try_from(value: v1::EnvResolveResult) -> Result<Self> {
        Ok(Self {
            vars: value
                .vars
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_>>()?,
        })
    }
}
impl From<model::EnvResolveResult> for v1::EnvResolveResult {
    fn from(value: model::EnvResolveResult) -> Self {
        Self {
            vars: value.vars.into_iter().map(Into::into).collect(),
        }
    }
}
impl TryFrom<v1::EnvPair> for model::EnvPair {
    type Error = Error;
    fn try_from(value: v1::EnvPair) -> Result<Self> {
        Ok(Self {
            key: value.key,
            value: value.value,
        })
    }
}
impl From<model::EnvPair> for v1::EnvPair {
    fn from(value: model::EnvPair) -> Self {
        Self {
            key: value.key,
            value: value.value,
        }
    }
}
impl TryFrom<v1::EnvFormatParams> for model::EnvFormatParams {
    type Error = Error;
    fn try_from(value: v1::EnvFormatParams) -> Result<Self> {
        Ok(Self {
            profile_id: value.profile_id,
            entry_ids: value.entry_ids.map(|value| value.items),
            format: model::EnvFormat::try_from(value.format)?,
        })
    }
}
impl From<model::EnvFormatParams> for v1::EnvFormatParams {
    fn from(value: model::EnvFormatParams) -> Self {
        Self {
            profile_id: value.profile_id,
            entry_ids: value
                .entry_ids
                .map(|value| v1::OptionalStringList { items: value }),
            format: value.format.into(),
        }
    }
}
impl TryFrom<v1::TextResult> for model::TextResult {
    type Error = Error;
    fn try_from(value: v1::TextResult) -> Result<Self> {
        Ok(Self { text: value.text })
    }
}
impl From<model::TextResult> for v1::TextResult {
    fn from(value: model::TextResult) -> Self {
        Self { text: value.text }
    }
}
impl TryFrom<v1::EnvParseParams> for model::EnvParseParams {
    type Error = Error;
    fn try_from(value: v1::EnvParseParams) -> Result<Self> {
        Ok(Self { text: value.text })
    }
}
impl From<model::EnvParseParams> for v1::EnvParseParams {
    fn from(value: model::EnvParseParams) -> Self {
        Self { text: value.text }
    }
}
impl TryFrom<v1::TotpCode> for model::TotpCode {
    type Error = Error;
    fn try_from(value: v1::TotpCode) -> Result<Self> {
        Ok(Self {
            code: value.code,
            expires_in: value.expires_in,
            period: value.period,
        })
    }
}
impl From<model::TotpCode> for v1::TotpCode {
    fn from(value: model::TotpCode) -> Self {
        Self {
            code: value.code,
            expires_in: value.expires_in,
            period: value.period,
        }
    }
}
impl TryFrom<v1::PinnedTotp> for model::PinnedTotp {
    type Error = Error;
    fn try_from(value: v1::PinnedTotp) -> Result<Self> {
        Ok(Self {
            id: value.id,
            profile_name: value.profile_name,
            name: value.name,
        })
    }
}
impl From<model::PinnedTotp> for v1::PinnedTotp {
    fn from(value: model::PinnedTotp) -> Self {
        Self {
            id: value.id,
            profile_name: value.profile_name,
            name: value.name,
        }
    }
}
impl TryFrom<v1::ClipboardCopyParams> for model::ClipboardCopyParams {
    type Error = Error;
    fn try_from(value: v1::ClipboardCopyParams) -> Result<Self> {
        Ok(Self {
            id: value.id,
            field: value.field,
            clear_after_secs: value.clear_after_secs,
        })
    }
}
impl From<model::ClipboardCopyParams> for v1::ClipboardCopyParams {
    fn from(value: model::ClipboardCopyParams) -> Self {
        Self {
            id: value.id,
            field: value.field,
            clear_after_secs: value.clear_after_secs,
        }
    }
}
impl TryFrom<v1::ClipboardCopyResult> for model::ClipboardCopyResult {
    type Error = Error;
    fn try_from(value: v1::ClipboardCopyResult) -> Result<Self> {
        Ok(Self {
            clear_after_secs: value.clear_after_secs,
        })
    }
}
impl From<model::ClipboardCopyResult> for v1::ClipboardCopyResult {
    fn from(value: model::ClipboardCopyResult) -> Self {
        Self {
            clear_after_secs: value.clear_after_secs,
        }
    }
}
impl TryFrom<v1::BrowserLaunchParams> for model::BrowserLaunchParams {
    type Error = Error;
    fn try_from(value: v1::BrowserLaunchParams) -> Result<Self> {
        Ok(Self {
            profile_id: value.profile_id,
            url: value.url,
        })
    }
}
impl From<model::BrowserLaunchParams> for v1::BrowserLaunchParams {
    fn from(value: model::BrowserLaunchParams) -> Self {
        Self {
            profile_id: value.profile_id,
            url: value.url,
        }
    }
}
impl TryFrom<v1::BrowserSession> for model::BrowserSession {
    type Error = Error;
    fn try_from(value: v1::BrowserSession) -> Result<Self> {
        Ok(Self {
            profile_id: value.profile_id,
            engine: model::BrowserEngine::try_from(value.engine)?,
            pid: value.pid,
            started_at: value.started_at,
            user_data_dir: value.user_data_dir,
        })
    }
}
impl From<model::BrowserSession> for v1::BrowserSession {
    fn from(value: model::BrowserSession) -> Self {
        Self {
            profile_id: value.profile_id,
            engine: value.engine.into(),
            pid: value.pid,
            started_at: value.started_at,
            user_data_dir: value.user_data_dir,
        }
    }
}
impl TryFrom<v1::DetectedBrowser> for model::DetectedBrowser {
    type Error = Error;
    fn try_from(value: v1::DetectedBrowser) -> Result<Self> {
        Ok(Self {
            engine: model::BrowserEngine::try_from(value.engine)?,
            path: value.path,
            version: value.version,
        })
    }
}
impl From<model::DetectedBrowser> for v1::DetectedBrowser {
    fn from(value: model::DetectedBrowser) -> Self {
        Self {
            engine: value.engine.into(),
            path: value.path,
            version: value.version,
        }
    }
}
impl TryFrom<v1::SessionArchiveInfo> for model::SessionArchiveInfo {
    type Error = Error;
    fn try_from(value: v1::SessionArchiveInfo) -> Result<Self> {
        Ok(Self {
            exists: value.exists,
            size_bytes: value.size_bytes,
            updated_at: value.updated_at,
            committed: value.committed,
        })
    }
}
impl From<model::SessionArchiveInfo> for v1::SessionArchiveInfo {
    fn from(value: model::SessionArchiveInfo) -> Self {
        Self {
            exists: value.exists,
            size_bytes: value.size_bytes,
            updated_at: value.updated_at,
            committed: value.committed,
        }
    }
}
impl TryFrom<v1::SyncStatus> for model::SyncStatus {
    type Error = Error;
    fn try_from(value: v1::SyncStatus) -> Result<Self> {
        Ok(Self {
            has_remote: value.has_remote,
            remote_url: value.remote_url,
            branch: value.branch,
            ahead: value.ahead,
            behind: value.behind,
            dirty: value.dirty,
            last_sync_at: value.last_sync_at,
            conflicts: value
                .conflicts
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_>>()?,
            log: value
                .log
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_>>()?,
        })
    }
}
impl From<model::SyncStatus> for v1::SyncStatus {
    fn from(value: model::SyncStatus) -> Self {
        Self {
            has_remote: value.has_remote,
            remote_url: value.remote_url,
            branch: value.branch,
            ahead: value.ahead,
            behind: value.behind,
            dirty: value.dirty,
            last_sync_at: value.last_sync_at,
            conflicts: value.conflicts.into_iter().map(Into::into).collect(),
            log: value.log.into_iter().map(Into::into).collect(),
        }
    }
}
impl TryFrom<v1::CommitInfo> for model::CommitInfo {
    type Error = Error;
    fn try_from(value: v1::CommitInfo) -> Result<Self> {
        Ok(Self {
            commit: value.commit,
            at: value.at,
            author: value.author,
            message: value.message,
        })
    }
}
impl From<model::CommitInfo> for v1::CommitInfo {
    fn from(value: model::CommitInfo) -> Self {
        Self {
            commit: value.commit,
            at: value.at,
            author: value.author,
            message: value.message,
        }
    }
}
impl TryFrom<v1::SyncParams> for model::SyncParams {
    type Error = Error;
    fn try_from(value: v1::SyncParams) -> Result<Self> {
        Ok(Self {
            mode: model::SyncMode::try_from(value.mode)?,
        })
    }
}
impl From<model::SyncParams> for v1::SyncParams {
    fn from(value: model::SyncParams) -> Self {
        Self {
            mode: value.mode.into(),
        }
    }
}
impl TryFrom<v1::SyncResult> for model::SyncResult {
    type Error = Error;
    fn try_from(value: v1::SyncResult) -> Result<Self> {
        Ok(Self {
            pulled: value.pulled,
            pushed: value.pushed,
            conflicts: value
                .conflicts
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_>>()?,
            status: value
                .status
                .ok_or_else(|| invalid("SyncResult.status"))?
                .try_into()?,
        })
    }
}
impl From<model::SyncResult> for v1::SyncResult {
    fn from(value: model::SyncResult) -> Self {
        Self {
            pulled: value.pulled,
            pushed: value.pushed,
            conflicts: value.conflicts.into_iter().map(Into::into).collect(),
            status: Some(value.status.into()),
        }
    }
}
impl TryFrom<v1::ConflictSummary> for model::ConflictSummary {
    type Error = Error;
    fn try_from(value: v1::ConflictSummary) -> Result<Self> {
        Ok(Self {
            entry_id: value.entry_id,
            profile_id: value.profile_id,
            name: value.name,
        })
    }
}
impl From<model::ConflictSummary> for v1::ConflictSummary {
    fn from(value: model::ConflictSummary) -> Self {
        Self {
            entry_id: value.entry_id,
            profile_id: value.profile_id,
            name: value.name,
        }
    }
}
impl TryFrom<v1::ConflictDetail> for model::ConflictDetail {
    type Error = Error;
    fn try_from(value: v1::ConflictDetail) -> Result<Self> {
        Ok(Self {
            entry_id: value.entry_id,
            mine: value.mine.map(TryInto::try_into).transpose()?,
            theirs: value.theirs.map(TryInto::try_into).transpose()?,
        })
    }
}
impl From<model::ConflictDetail> for v1::ConflictDetail {
    fn from(value: model::ConflictDetail) -> Self {
        Self {
            entry_id: value.entry_id,
            mine: value.mine.map(Into::into),
            theirs: value.theirs.map(Into::into),
        }
    }
}
impl TryFrom<v1::ResolveParams> for model::ResolveParams {
    type Error = Error;
    fn try_from(value: v1::ResolveParams) -> Result<Self> {
        Ok(Self {
            entry_id: value.entry_id,
            resolution: model::Resolution::try_from(value.resolution)?,
            merged: value.merged.map(TryInto::try_into).transpose()?,
        })
    }
}
impl From<model::ResolveParams> for v1::ResolveParams {
    fn from(value: model::ResolveParams) -> Self {
        Self {
            entry_id: value.entry_id,
            resolution: value.resolution.into(),
            merged: value.merged.map(Into::into),
        }
    }
}
impl TryFrom<v1::SetRemoteParams> for model::SetRemoteParams {
    type Error = Error;
    fn try_from(value: v1::SetRemoteParams) -> Result<Self> {
        Ok(Self { url: value.url })
    }
}
impl From<model::SetRemoteParams> for v1::SetRemoteParams {
    fn from(value: model::SetRemoteParams) -> Self {
        Self { url: value.url }
    }
}
impl TryFrom<v1::AddRecipientParams> for model::AddRecipientParams {
    type Error = Error;
    fn try_from(value: v1::AddRecipientParams) -> Result<Self> {
        Ok(Self {
            label: value.label,
            public_key: value.public_key,
        })
    }
}
impl From<model::AddRecipientParams> for v1::AddRecipientParams {
    fn from(value: model::AddRecipientParams) -> Self {
        Self {
            label: value.label,
            public_key: value.public_key,
        }
    }
}
impl TryFrom<v1::RemoveRecipientParams> for model::RemoveRecipientParams {
    type Error = Error;
    fn try_from(value: v1::RemoveRecipientParams) -> Result<Self> {
        Ok(Self { label: value.label })
    }
}
impl From<model::RemoveRecipientParams> for v1::RemoveRecipientParams {
    fn from(value: model::RemoveRecipientParams) -> Self {
        Self { label: value.label }
    }
}
impl TryFrom<v1::ImportParams> for model::ImportParams {
    type Error = Error;
    fn try_from(value: v1::ImportParams) -> Result<Self> {
        Ok(Self {
            format: model::ImportFormat::try_from(value.format)?,
            path: value.path,
            profile_id: value.profile_id,
            skip_duplicates: value.skip_duplicates,
            passphrase: value.passphrase,
        })
    }
}
impl From<model::ImportParams> for v1::ImportParams {
    fn from(value: model::ImportParams) -> Self {
        Self {
            format: value.format.into(),
            path: value.path,
            profile_id: value.profile_id,
            skip_duplicates: value.skip_duplicates,
            passphrase: value.passphrase,
        }
    }
}
impl TryFrom<v1::ImportPreview> for model::ImportPreview {
    type Error = Error;
    fn try_from(value: v1::ImportPreview) -> Result<Self> {
        Ok(Self {
            items: value
                .items
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_>>()?,
            warnings: value.warnings,
        })
    }
}
impl From<model::ImportPreview> for v1::ImportPreview {
    fn from(value: model::ImportPreview) -> Self {
        Self {
            items: value.items.into_iter().map(Into::into).collect(),
            warnings: value.warnings,
        }
    }
}
impl TryFrom<v1::ImportItem> for model::ImportItem {
    type Error = Error;
    fn try_from(value: v1::ImportItem) -> Result<Self> {
        Ok(Self {
            name: value.name,
            entry_type: model::EntryType::try_from(value.r#type)?,
            profile_name: value.profile_name,
            duplicate: value.duplicate,
        })
    }
}
impl From<model::ImportItem> for v1::ImportItem {
    fn from(value: model::ImportItem) -> Self {
        Self {
            name: value.name,
            r#type: value.entry_type.into(),
            profile_name: value.profile_name,
            duplicate: value.duplicate,
        }
    }
}
impl TryFrom<v1::ImportApplyResult> for model::ImportApplyResult {
    type Error = Error;
    fn try_from(value: v1::ImportApplyResult) -> Result<Self> {
        Ok(Self {
            created: value.created,
            skipped: value.skipped,
            profiles_created: value.profiles_created,
        })
    }
}
impl From<model::ImportApplyResult> for v1::ImportApplyResult {
    fn from(value: model::ImportApplyResult) -> Self {
        Self {
            created: value.created,
            skipped: value.skipped,
            profiles_created: value.profiles_created,
        }
    }
}
impl TryFrom<v1::ExportParams> for model::ExportParams {
    type Error = Error;
    fn try_from(value: v1::ExportParams) -> Result<Self> {
        Ok(Self {
            format: model::ExportFormat::try_from(value.format)?,
            path: value.path,
            profile_id: value.profile_id,
            passphrase: value.passphrase,
        })
    }
}
impl From<model::ExportParams> for v1::ExportParams {
    fn from(value: model::ExportParams) -> Self {
        Self {
            format: value.format.into(),
            path: value.path,
            profile_id: value.profile_id,
            passphrase: value.passphrase,
        }
    }
}
impl TryFrom<v1::ExportResult> for model::ExportResult {
    type Error = Error;
    fn try_from(value: v1::ExportResult) -> Result<Self> {
        Ok(Self {
            path: value.path,
            entries: value.entries,
        })
    }
}
impl From<model::ExportResult> for v1::ExportResult {
    fn from(value: model::ExportResult) -> Self {
        Self {
            path: value.path,
            entries: value.entries,
        }
    }
}
impl TryFrom<v1::DoctorParams> for model::DoctorParams {
    type Error = Error;
    fn try_from(value: v1::DoctorParams) -> Result<Self> {
        Ok(Self {
            fix: value.fix,
            rebuild_manifest: value.rebuild_manifest,
            staged: value.staged,
        })
    }
}
impl From<model::DoctorParams> for v1::DoctorParams {
    fn from(value: model::DoctorParams) -> Self {
        Self {
            fix: value.fix,
            rebuild_manifest: value.rebuild_manifest,
            staged: value.staged,
        }
    }
}
impl TryFrom<v1::DoctorReport> for model::DoctorReport {
    type Error = Error;
    fn try_from(value: v1::DoctorReport) -> Result<Self> {
        Ok(Self {
            ok: value.ok,
            issues: value
                .issues
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_>>()?,
            fixed: value.fixed,
        })
    }
}
impl From<model::DoctorReport> for v1::DoctorReport {
    fn from(value: model::DoctorReport) -> Self {
        Self {
            ok: value.ok,
            issues: value.issues.into_iter().map(Into::into).collect(),
            fixed: value.fixed,
        }
    }
}
impl TryFrom<v1::DoctorIssue> for model::DoctorIssue {
    type Error = Error;
    fn try_from(value: v1::DoctorIssue) -> Result<Self> {
        Ok(Self {
            severity: model::Severity::try_from(value.severity)?,
            code: value.code,
            message: value.message,
            path: value.path,
        })
    }
}
impl From<model::DoctorIssue> for v1::DoctorIssue {
    fn from(value: model::DoctorIssue) -> Self {
        Self {
            severity: value.severity.into(),
            code: value.code,
            message: value.message,
            path: value.path,
        }
    }
}
impl TryFrom<v1::Settings> for model::Settings {
    type Error = Error;
    fn try_from(value: v1::Settings) -> Result<Self> {
        Ok(Self {
            auto_lock_secs: value.auto_lock_secs,
            clipboard_clear_secs: value.clipboard_clear_secs,
            reveal_hide_secs: value.reveal_hide_secs,
            lock_on_screen_lock: value.lock_on_screen_lock,
            tray_enabled: value.tray_enabled,
            autostart: value.autostart,
            theme: model::Theme::try_from(value.theme)?,
            global_shortcut: value.global_shortcut,
            keep_browsers_open_on_lock: value.keep_browsers_open_on_lock,
            names_in_commits: value.names_in_commits,
            auto_push: value.auto_push,
            sessions_commit: value.sessions_commit,
            git_name: value.git_name,
            git_email: value.git_email,
        })
    }
}
impl From<model::Settings> for v1::Settings {
    fn from(value: model::Settings) -> Self {
        Self {
            auto_lock_secs: value.auto_lock_secs,
            clipboard_clear_secs: value.clipboard_clear_secs,
            reveal_hide_secs: value.reveal_hide_secs,
            lock_on_screen_lock: value.lock_on_screen_lock,
            tray_enabled: value.tray_enabled,
            autostart: value.autostart,
            theme: value.theme.into(),
            global_shortcut: value.global_shortcut,
            keep_browsers_open_on_lock: value.keep_browsers_open_on_lock,
            names_in_commits: value.names_in_commits,
            auto_push: value.auto_push,
            sessions_commit: value.sessions_commit,
            git_name: value.git_name,
            git_email: value.git_email,
        }
    }
}
impl TryFrom<v1::EntryPayload> for model::EntryPayload {
    type Error = Error;
    fn try_from(value: v1::EntryPayload) -> Result<Self> {
        use v1::entry_payload::Data;
        Ok(
            match value.data.ok_or_else(|| invalid("EntryPayload.data"))? {
                Data::Login(value) => Self::Login(value.try_into()?),
                Data::Secret(value) => Self::Secret(value.try_into()?),
                Data::Env(value) => Self::Env(value.try_into()?),
                Data::File(value) => Self::File(value.try_into()?),
                Data::Note(value) => Self::Note(value.try_into()?),
                Data::SshKey(value) => Self::SshKey(value.try_into()?),
                Data::Database(value) => Self::Database(value.try_into()?),
                Data::ApiKey(value) => Self::ApiKey(value.try_into()?),
            },
        )
    }
}
impl From<model::EntryPayload> for v1::EntryPayload {
    fn from(value: model::EntryPayload) -> Self {
        use v1::entry_payload::Data;
        Self {
            data: Some(match value {
                model::EntryPayload::Login(value) => Data::Login(value.into()),
                model::EntryPayload::Secret(value) => Data::Secret(value.into()),
                model::EntryPayload::Env(value) => Data::Env(value.into()),
                model::EntryPayload::File(value) => Data::File(value.into()),
                model::EntryPayload::Note(value) => Data::Note(value.into()),
                model::EntryPayload::SshKey(value) => Data::SshKey(value.into()),
                model::EntryPayload::Database(value) => Data::Database(value.into()),
                model::EntryPayload::ApiKey(value) => Data::ApiKey(value.into()),
            }),
        }
    }
}
impl TryFrom<v1::DomainRequest> for model::DomainRequest {
    type Error = Error;
    fn try_from(value: v1::DomainRequest) -> Result<Self> {
        use v1::domain_request::Operation;
        Ok(
            match value
                .operation
                .ok_or_else(|| invalid("DomainRequest.operation"))?
            {
                Operation::ValidateEntry(value) => Self::ValidateEntry(value.try_into()?),
                Operation::ValidateProfile(value) => Self::ValidateProfile(value.try_into()?),
                Operation::ValidateRecipient(value) => Self::ValidateRecipient(value.try_into()?),
                Operation::RecipientKeyKind(value) => Self::RecipientKeyKind(value.value),
                Operation::ParseDotenv(value) => Self::ParseDotenv(value.value),
                Operation::FormatEnv(value) => Self::FormatEnv(value.try_into()?),
                Operation::ParseOtpauth(value) => Self::ParseOtpauth(value.value),
                Operation::GeneratePassword(value) => Self::GeneratePassword(value.value),
                Operation::EstimateStrength(value) => Self::EstimateStrength(value.value),
                Operation::EntryDefaults(value) => Self::EntryDefaults(value.try_into()?),
                Operation::DatabaseUrl(value) => Self::DatabaseUrl(value.try_into()?),
                Operation::SecretPaths(value) => Self::SecretPaths(value.try_into()?),
                Operation::ReadField(value) => Self::ReadField(value.try_into()?),
                Operation::RedactPayload(value) => Self::RedactPayload(value.try_into()?),
                Operation::MergeKeepOnNone(value) => Self::MergeKeepOnNone(value.try_into()?),
                Operation::ApplyOptionalClears(value) => {
                    Self::ApplyOptionalClears(value.try_into()?)
                }
                Operation::FlattenEntry(value) => Self::FlattenEntry(value.try_into()?),
                Operation::MergeEntries(value) => Self::MergeEntries(value.try_into()?),
                Operation::DefaultBrowserSettings(_) => Self::DefaultBrowserSettings(()),
                Operation::FileDigest(value) => {
                    Self::FileDigest(value.value.map(TryInto::try_into).transpose()?)
                }
                Operation::DiffRows(value) => Self::DiffRows(value.try_into()?),
            },
        )
    }
}
impl From<model::DomainRequest> for v1::DomainRequest {
    fn from(value: model::DomainRequest) -> Self {
        use v1::domain_request::Operation;
        Self {
            operation: Some(match value {
                model::DomainRequest::ValidateEntry(value) => {
                    Operation::ValidateEntry(value.into())
                }
                model::DomainRequest::ValidateProfile(value) => {
                    Operation::ValidateProfile(value.into())
                }
                model::DomainRequest::ValidateRecipient(value) => {
                    Operation::ValidateRecipient(value.into())
                }
                model::DomainRequest::RecipientKeyKind(value) => {
                    Operation::RecipientKeyKind(v1::StringValue { value })
                }
                model::DomainRequest::ParseDotenv(value) => {
                    Operation::ParseDotenv(v1::StringValue { value })
                }
                model::DomainRequest::FormatEnv(value) => Operation::FormatEnv(value.into()),
                model::DomainRequest::ParseOtpauth(value) => {
                    Operation::ParseOtpauth(v1::StringValue { value })
                }
                model::DomainRequest::GeneratePassword(value) => {
                    Operation::GeneratePassword(v1::UInt32Value { value })
                }
                model::DomainRequest::EstimateStrength(value) => {
                    Operation::EstimateStrength(v1::StringValue { value })
                }
                model::DomainRequest::EntryDefaults(value) => {
                    Operation::EntryDefaults(value.into())
                }
                model::DomainRequest::DatabaseUrl(value) => Operation::DatabaseUrl(value.into()),
                model::DomainRequest::SecretPaths(value) => Operation::SecretPaths(value.into()),
                model::DomainRequest::ReadField(value) => Operation::ReadField(value.into()),
                model::DomainRequest::RedactPayload(value) => {
                    Operation::RedactPayload(value.into())
                }
                model::DomainRequest::MergeKeepOnNone(value) => {
                    Operation::MergeKeepOnNone(value.into())
                }
                model::DomainRequest::ApplyOptionalClears(value) => {
                    Operation::ApplyOptionalClears(value.into())
                }
                model::DomainRequest::FlattenEntry(value) => Operation::FlattenEntry(value.into()),
                model::DomainRequest::MergeEntries(value) => Operation::MergeEntries(value.into()),
                model::DomainRequest::DefaultBrowserSettings(()) => {
                    Operation::DefaultBrowserSettings(v1::Empty {})
                }
                model::DomainRequest::FileDigest(value) => {
                    Operation::FileDigest(v1::OptionalEntryInput {
                        value: value.map(Into::into),
                    })
                }
                model::DomainRequest::DiffRows(value) => Operation::DiffRows(value.into()),
            }),
        }
    }
}
impl TryFrom<v1::DomainResponse> for model::DomainResponse {
    type Error = Error;
    fn try_from(value: v1::DomainResponse) -> Result<Self> {
        use v1::domain_response::Operation;
        Ok(
            match value
                .operation
                .ok_or_else(|| invalid("DomainResponse.operation"))?
            {
                Operation::ValidateEntry(value) => {
                    Self::ValidateEntry(value.items.into_iter().collect())
                }
                Operation::ValidateProfile(value) => {
                    Self::ValidateProfile(value.items.into_iter().collect())
                }
                Operation::ValidateRecipient(value) => {
                    Self::ValidateRecipient(value.items.into_iter().collect())
                }
                Operation::RecipientKeyKind(value) => Self::RecipientKeyKind(
                    value
                        .value
                        .map(model::DomainRecipientKeyKind::try_from)
                        .transpose()?,
                ),
                Operation::ParseDotenv(value) => Self::ParseDotenv(
                    value
                        .items
                        .into_iter()
                        .map(TryInto::try_into)
                        .collect::<Result<_>>()?,
                ),
                Operation::FormatEnv(value) => Self::FormatEnv(value.value),
                Operation::ParseOtpauth(value) => {
                    Self::ParseOtpauth(value.value.map(TryInto::try_into).transpose()?)
                }
                Operation::GeneratePassword(value) => Self::GeneratePassword(value.value),
                Operation::EstimateStrength(value) => Self::EstimateStrength(value.try_into()?),
                Operation::EntryDefaults(value) => Self::EntryDefaults(value.try_into()?),
                Operation::DatabaseUrl(value) => Self::DatabaseUrl(value.value),
                Operation::SecretPaths(value) => Self::SecretPaths(value.items),
                Operation::ReadField(value) => Self::ReadField(value.value),
                Operation::RedactPayload(value) => Self::RedactPayload(value.try_into()?),
                Operation::MergeKeepOnNone(value) => Self::MergeKeepOnNone(value.try_into()?),
                Operation::ApplyOptionalClears(value) => {
                    Self::ApplyOptionalClears(value.try_into()?)
                }
                Operation::FlattenEntry(value) => {
                    Self::FlattenEntry(value.items.into_iter().collect())
                }
                Operation::MergeEntries(value) => Self::MergeEntries(value.try_into()?),
                Operation::DefaultBrowserSettings(value) => {
                    Self::DefaultBrowserSettings(value.try_into()?)
                }
                Operation::FileDigest(value) => Self::FileDigest(value.value),
                Operation::DiffRows(value) => Self::DiffRows(
                    value
                        .items
                        .into_iter()
                        .map(TryInto::try_into)
                        .collect::<Result<_>>()?,
                ),
            },
        )
    }
}
impl From<model::DomainResponse> for v1::DomainResponse {
    fn from(value: model::DomainResponse) -> Self {
        use v1::domain_response::Operation;
        Self {
            operation: Some(match value {
                model::DomainResponse::ValidateEntry(value) => {
                    Operation::ValidateEntry(v1::StringMap {
                        items: value.into_iter().collect(),
                    })
                }
                model::DomainResponse::ValidateProfile(value) => {
                    Operation::ValidateProfile(v1::StringMap {
                        items: value.into_iter().collect(),
                    })
                }
                model::DomainResponse::ValidateRecipient(value) => {
                    Operation::ValidateRecipient(v1::StringMap {
                        items: value.into_iter().collect(),
                    })
                }
                model::DomainResponse::RecipientKeyKind(value) => {
                    Operation::RecipientKeyKind(v1::OptionalDomainRecipientKeyKind {
                        value: value.map(Into::into),
                    })
                }
                model::DomainResponse::ParseDotenv(value) => {
                    Operation::ParseDotenv(v1::DomainEnvVarList {
                        items: value.into_iter().map(Into::into).collect(),
                    })
                }
                model::DomainResponse::FormatEnv(value) => {
                    Operation::FormatEnv(v1::StringValue { value })
                }
                model::DomainResponse::ParseOtpauth(value) => {
                    Operation::ParseOtpauth(v1::OptionalParsedOtpauth {
                        value: value.map(Into::into),
                    })
                }
                model::DomainResponse::GeneratePassword(value) => {
                    Operation::GeneratePassword(v1::StringValue { value })
                }
                model::DomainResponse::EstimateStrength(value) => {
                    Operation::EstimateStrength(value.into())
                }
                model::DomainResponse::EntryDefaults(value) => {
                    Operation::EntryDefaults(value.into())
                }
                model::DomainResponse::DatabaseUrl(value) => {
                    Operation::DatabaseUrl(v1::StringValue { value })
                }
                model::DomainResponse::SecretPaths(value) => {
                    Operation::SecretPaths(v1::DomainStringList { items: value })
                }
                model::DomainResponse::ReadField(value) => {
                    Operation::ReadField(v1::OptionalString { value })
                }
                model::DomainResponse::RedactPayload(value) => {
                    Operation::RedactPayload(value.into())
                }
                model::DomainResponse::MergeKeepOnNone(value) => {
                    Operation::MergeKeepOnNone(value.into())
                }
                model::DomainResponse::ApplyOptionalClears(value) => {
                    Operation::ApplyOptionalClears(value.into())
                }
                model::DomainResponse::FlattenEntry(value) => {
                    Operation::FlattenEntry(v1::StringMap {
                        items: value.into_iter().collect(),
                    })
                }
                model::DomainResponse::MergeEntries(value) => Operation::MergeEntries(value.into()),
                model::DomainResponse::DefaultBrowserSettings(value) => {
                    Operation::DefaultBrowserSettings(value.into())
                }
                model::DomainResponse::FileDigest(value) => {
                    Operation::FileDigest(v1::OptionalString { value })
                }
                model::DomainResponse::DiffRows(value) => {
                    Operation::DiffRows(v1::DomainDiffRowList {
                        items: value.into_iter().map(Into::into).collect(),
                    })
                }
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_messages_and_oneofs_fail_closed() {
        assert!(model::EntryInput::try_from(v1::EntryInput::default()).is_err());
        assert!(model::ProfileInput::try_from(v1::ProfileInput::default()).is_err());
        assert!(model::ActionSaveParams::try_from(v1::ActionSaveParams::default()).is_err());
        assert!(model::EntryPayload::try_from(v1::EntryPayload::default()).is_err());
        assert!(model::DomainRequest::try_from(v1::DomainRequest::default()).is_err());
        assert!(model::DomainResponse::try_from(v1::DomainResponse::default()).is_err());
        let invalid = v1::DomainRequest {
            operation: Some(v1::domain_request::Operation::ReadField(
                v1::ReadFieldInput {
                    payload: None,
                    path: "password".into(),
                },
            )),
        };
        assert!(model::DomainRequest::try_from(invalid).is_err());
    }

    #[test]
    fn unknown_and_unspecified_enums_are_rejected() {
        for invalid in [0, -1, i32::MAX] {
            assert!(model::ClientKind::try_from(invalid).is_err());
            assert!(model::EntryType::try_from(invalid).is_err());
            assert!(model::Theme::try_from(invalid).is_err());
            assert!(model::TotpAlgorithm::try_from(invalid).is_err());
            assert!(model::DomainRecipientKeyKind::try_from(invalid).is_err());
        }
        let request = v1::ListEntriesParams {
            r#type: Some(0),
            ..Default::default()
        };
        assert!(model::ListEntriesParams::try_from(request).is_err());
        let request = v1::DomainRequest {
            operation: Some(v1::domain_request::Operation::EntryDefaults(
                v1::EntryDefaultsInput {
                    profile_id: "profile".into(),
                    entry_type: 999,
                },
            )),
        };
        assert!(model::DomainRequest::try_from(request).is_err());
    }

    #[test]
    fn managed_firefox_is_the_only_browser_engine() {
        assert_eq!(v1::BrowserEngine::Firefox as i32, 1);
        assert_eq!(
            model::BrowserEngine::try_from(1).unwrap(),
            model::BrowserEngine::Firefox
        );
        for invalid in [0, 2, 3, 4, 5, -1, i32::MAX] {
            assert!(model::BrowserEngine::try_from(invalid).is_err());
        }
    }

    #[test]
    fn narrow_integer_fields_are_checked() {
        let totp = v1::TotpConfig {
            algorithm: v1::TotpAlgorithm::Sha1 as i32,
            digits: 256,
            ..Default::default()
        };
        assert!(model::TotpConfig::try_from(totp).is_err());
        let database = v1::DatabaseData {
            port: Some(u16::MAX as u32 + 1),
            password: Some("secret-marker".into()),
            ..Default::default()
        };
        let error = model::DatabaseData::try_from(database).unwrap_err();
        assert!(!error.to_string().contains("secret-marker"));
        let strength = v1::PasswordStrength {
            score: 256,
            ..Default::default()
        };
        assert!(model::PasswordStrength::try_from(strength).is_err());
        let database = v1::DatabaseData {
            port: Some(u16::MAX as u32),
            ..Default::default()
        };
        assert_eq!(
            model::DatabaseData::try_from(database).unwrap().port,
            Some(u16::MAX)
        );
    }

    #[test]
    fn secret_absence_and_explicit_clear_survive_conversion() {
        for password in [None, Some(String::new()), Some("secret-marker".into())] {
            let input = model::EntryPayload::Login(model::LoginData {
                username: "user".into(),
                password,
                urls: vec![],
                totp: None,
                custom: vec![],
            });
            let wire = v1::EntryPayload::from(input.clone());
            assert!(!format!("{input:?}").contains("secret-marker"));
            assert!(!format!("{wire:?}").contains("secret-marker"));
            assert_eq!(model::EntryPayload::try_from(wire).unwrap(), input);
        }
    }

    #[test]
    fn optional_lists_preserve_absent_and_present_empty() {
        for entry_ids in [None, Some(vec![]), Some(vec!["entry".into()])] {
            let input = model::EnvResolveParams {
                profile_id: "profile".into(),
                entry_ids,
            };
            let wire = v1::EnvResolveParams::from(input.clone());
            assert_eq!(model::EnvResolveParams::try_from(wire).unwrap(), input);
        }
        let request = model::DomainRequest::FileDigest(None);
        let wire = v1::DomainRequest::from(request.clone());
        assert_eq!(model::DomainRequest::try_from(wire).unwrap(), request);
    }
}
