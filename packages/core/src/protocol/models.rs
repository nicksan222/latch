//! Core-owned values used by vault operations and persistence.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainRecipientKeyKind {
    Age,
    Ssh,
    Plugin,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientKind {
    Cli,
    Desktop,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentEventKind {
    Locked,
    Unlocked,
    VaultChanged,
    ProfilesChanged,
    EntriesChanged,
    BrowserChanged,
    SyncChanged,
    LockWarning,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecipientKind {
    Passphrase,
    Age,
    Ssh,
    Plugin,
    Recovery,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserEngine {
    Firefox,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutofillScope {
    All,
    Selected,
    None,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryType {
    Login,
    Secret,
    Env,
    File,
    Note,
    SshKey,
    Database,
    ApiKey,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TotpAlgorithm {
    #[serde(rename = "SHA1")]
    Sha1,
    #[serde(rename = "SHA256")]
    Sha256,
    #[serde(rename = "SHA512")]
    Sha512,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvFormat {
    Dotenv,
    Export,
    Json,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VaultTrustState {
    FirstUse,
    Pinned,
    Stale,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    Both,
    PullOnly,
    PushOnly,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    Mine,
    Theirs,
    Merged,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportFormat {
    Dotenv,
    Bitwarden,
    OnePasswordCsv,
    KeepassXml,
    BrowserCsv,
    Pass,
    Json,
    Yaml,
    LatchArchive,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    LatchArchive,
    Json,
    Dotenv,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    System,
    Light,
    Dark,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookParams {
    pub path: String,
    pub executable: Option<String>,
}
impl std::fmt::Debug for HookParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HookParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookResult {
    pub path: Option<String>,
    pub removed: bool,
}
impl std::fmt::Debug for HookResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HookResult([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainEntryTypeMetadata {
    #[serde(rename = "type")]
    pub r#type: EntryType,
    pub label: String,
    pub primary_copy_field: Option<String>,
    pub default_payload: EntryPayload,
}
impl std::fmt::Debug for DomainEntryTypeMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DomainEntryTypeMetadata([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainCatalog {
    pub entry_types: Vec<DomainEntryTypeMetadata>,
    pub default_browser_settings: BrowserSettings,
    pub default_settings: Settings,
    pub default_totp: TotpConfig,
}
impl std::fmt::Debug for DomainCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DomainCatalog([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedOtpauth {
    pub totp: TotpConfig,
    pub account: Option<String>,
}
impl std::fmt::Debug for ParsedOtpauth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ParsedOtpauth([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordStrength {
    pub score: u8,
    pub label: String,
    pub bits: u32,
    pub hints: Vec<String>,
}
impl std::fmt::Debug for PasswordStrength {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PasswordStrength([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryDefaultsInput {
    pub profile_id: String,
    pub entry_type: EntryType,
}
impl std::fmt::Debug for EntryDefaultsInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EntryDefaultsInput([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadFieldInput {
    pub payload: EntryPayload,
    pub path: String,
}
impl std::fmt::Debug for ReadFieldInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ReadFieldInput([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactedPayload {
    pub payload: EntryPayload,
    pub redacted: Vec<String>,
}
impl std::fmt::Debug for RedactedPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RedactedPayload([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadMergeInput {
    pub stored: EntryPayload,
    pub input: EntryPayload,
}
impl std::fmt::Debug for PayloadMergeInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PayloadMergeInput([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeEntriesInput {
    pub mine: EntryInput,
    pub theirs: EntryInput,
    pub from_theirs: Vec<String>,
}
impl std::fmt::Debug for MergeEntriesInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MergeEntriesInput([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainEnvFormatInput {
    pub vars: Vec<EnvPair>,
    pub format: EnvFormat,
}
impl std::fmt::Debug for DomainEnvFormatInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DomainEnvFormatInput([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateRecipientInput {
    pub label: String,
    pub key: String,
    pub existing: Vec<RecipientInfo>,
}
impl std::fmt::Debug for ValidateRecipientInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ValidateRecipientInput([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffRowsInput {
    pub mine: Option<EntryInput>,
    pub theirs: Option<EntryInput>,
}
impl std::fmt::Debug for DiffRowsInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DiffRowsInput([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainDiffRow {
    pub field: String,
    pub mine: String,
    pub theirs: String,
    pub differs: bool,
    pub content_differs: bool,
}
impl std::fmt::Debug for DomainDiffRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DomainDiffRow([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloParams {
    pub client: ClientKind,
    pub version: String,
}
impl std::fmt::Debug for HelloParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HelloParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloResult {
    pub agent_version: String,
    pub protocol: u32,
}
impl std::fmt::Debug for HelloResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HelloResult([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEvent {
    pub kind: AgentEventKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}
impl std::fmt::Debug for AgentEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AgentEvent([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub initialized: bool,
    pub locked: bool,
    pub vault_id: Option<String>,
    pub path: Option<String>,
    pub recipients: Vec<RecipientInfo>,
    pub dirty: bool,
    pub ahead: u32,
    pub behind: u32,
    pub has_remote: bool,
    pub last_sync_at: Option<String>,
    pub locks_in_secs: Option<u64>,
    pub trust: Option<TrustStatus>,
    pub integrity_problems: Vec<String>,
    pub rotation_pending: bool,
    pub memory_locked: Option<bool>,
}
impl std::fmt::Debug for VaultStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VaultStatus([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustStatus {
    pub state: VaultTrustState,
    pub fingerprint: String,
    pub key_seq: u64,
}
impl std::fmt::Debug for TrustStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TrustStatus([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgetTrustParams {
    #[serde(default)]
    pub confirm: bool,
}
impl std::fmt::Debug for ForgetTrustParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ForgetTrustParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipientInfo {
    pub label: String,
    pub kind: RecipientKind,
    pub public_key: String,
    pub added_at: Option<String>,
}
impl std::fmt::Debug for RecipientInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecipientInfo([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KdfParams {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}
impl std::fmt::Debug for KdfParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KdfParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitParams {
    pub path: String,
    pub passphrase: String,
    pub kdf: Option<KdfParams>,
    pub git_name: Option<String>,
    pub git_email: Option<String>,
    pub remote_url: Option<String>,
    pub recovery_kit: bool,
}
impl std::fmt::Debug for InitParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("InitParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitResult {
    pub status: VaultStatus,
    pub recovery_kit: Option<RecoveryKit>,
}
impl std::fmt::Debug for InitResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("InitResult([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryKit {
    pub vault_id: String,
    pub passphrase_public_key: String,
    pub recovery_public_key: String,
    pub recovery_secret_key: String,
    pub document: String,
}
impl std::fmt::Debug for RecoveryKit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecoveryKit([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneParams {
    pub url: String,
    pub path: String,
}
impl std::fmt::Debug for CloneParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CloneParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenParams {
    pub path: String,
}
impl std::fmt::Debug for OpenParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OpenParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownVault {
    pub path: String,
    pub vault_id: String,
    pub active: bool,
}
impl std::fmt::Debug for KnownVault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KnownVault([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlockParams {
    pub passphrase: Option<String>,
    pub identity_file: Option<String>,
    pub timeout_secs: Option<u64>,
}
impl std::fmt::Debug for UnlockParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UnlockParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KdfBenchmarkParams {
    pub target_ms: u32,
}
impl std::fmt::Debug for KdfBenchmarkParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KdfBenchmarkParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KdfBenchmark {
    pub params: KdfParams,
    pub measured_ms: u32,
}
impl std::fmt::Debug for KdfBenchmark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KdfBenchmark([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePassphraseParams {
    pub current: String,
    pub new_passphrase: String,
}
impl std::fmt::Debug for ChangePassphraseParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ChangePassphraseParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchAction {
    pub id: String,
    pub name: String,
    pub command: String,
    pub env_entry_ids: Vec<String>,
    pub env_keys: Option<Vec<String>>,
    pub urls: Vec<String>,
}
impl std::fmt::Debug for LaunchAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LaunchAction([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionInput {
    pub name: String,
    pub command: String,
    pub env_entry_ids: Vec<String>,
    pub env_keys: Option<Vec<String>>,
    pub urls: Vec<String>,
}
impl std::fmt::Debug for ActionInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ActionInput([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionSaveParams {
    pub profile_id: String,
    pub id: Option<String>,
    pub input: ActionInput,
}
impl std::fmt::Debug for ActionSaveParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ActionSaveParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionListResult {
    pub actions: Vec<LaunchAction>,
    pub working_directory: Option<String>,
}
impl std::fmt::Debug for ActionListResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ActionListResult([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionConfigureParams {
    pub profile_id: String,
    pub working_directory: Option<String>,
}
impl std::fmt::Debug for ActionConfigureParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ActionConfigureParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionIdParams {
    pub profile_id: String,
    pub action_id: String,
}
impl std::fmt::Debug for ActionIdParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ActionIdParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    pub id: String,
    pub name: String,
    pub color: String,
    pub icon: String,
    pub description: String,
    pub pinned: bool,
    pub entry_count: u32,
    pub browser_running: bool,
}
impl std::fmt::Debug for ProfileSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProfileSummary([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub color: String,
    pub icon: String,
    pub description: String,
    pub pinned: bool,
    pub browser: BrowserSettings,
    pub created_at: String,
    pub updated_at: String,
}
impl std::fmt::Debug for Profile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Profile([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserSettings {
    pub start_pages: Vec<String>,
    pub bookmarks: Vec<Bookmark>,
    pub autofill: AutofillScope,
    pub autofill_entry_ids: Vec<String>,
}
impl std::fmt::Debug for BrowserSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BrowserSettings([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub title: String,
    pub url: String,
}
impl std::fmt::Debug for Bookmark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Bookmark([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInput {
    pub name: String,
    pub color: String,
    pub icon: String,
    pub description: String,
    pub pinned: bool,
    pub browser: BrowserSettings,
}
impl std::fmt::Debug for ProfileInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProfileInput([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUpdateParams {
    pub id: String,
    pub input: ProfileInput,
}
impl std::fmt::Debug for ProfileUpdateParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProfileUpdateParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdParams {
    pub id: String,
}
impl std::fmt::Debug for IdParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("IdParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntrySummary {
    pub id: String,
    pub profile_id: String,
    #[serde(rename = "type")]
    pub entry_type: EntryType,
    pub name: String,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub subtitle: Option<String>,
    pub has_totp: bool,
    pub updated_at: String,
}
impl std::fmt::Debug for EntrySummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EntrySummary([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub profile_id: String,
    pub name: String,
    pub tags: Vec<String>,
    pub notes: String,
    pub favorite: bool,
    pub created_at: String,
    pub updated_at: String,
    pub payload: EntryPayload,
    pub redacted_fields: Vec<String>,
}
impl std::fmt::Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Entry([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryInput {
    pub profile_id: String,
    pub name: String,
    pub tags: Vec<String>,
    pub notes: String,
    pub favorite: bool,
    pub payload: EntryPayload,
}
impl std::fmt::Debug for EntryInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EntryInput([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryUpdateParams {
    pub id: String,
    pub input: EntryInput,
}
impl std::fmt::Debug for EntryUpdateParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EntryUpdateParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListEntriesParams {
    pub profile_id: Option<String>,
    pub query: Option<String>,
    #[serde(rename = "type")]
    pub entry_type: Option<EntryType>,
}
impl std::fmt::Debug for ListEntriesParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ListEntriesParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetEntryParams {
    pub id: String,
    pub reveal: bool,
}
impl std::fmt::Debug for GetEntryParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GetEntryParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealParams {
    pub id: String,
    pub field: String,
}
impl std::fmt::Debug for RevealParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RevealParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealResult {
    pub value: String,
}
impl std::fmt::Debug for RevealResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RevealResult([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveEntryParams {
    pub id: String,
    pub profile_id: String,
}
impl std::fmt::Debug for MoveEntryParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MoveEntryParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub commit: String,
    pub at: String,
    pub author: String,
    pub message: String,
    pub changed_fields: Vec<String>,
    pub deleted: bool,
}
impl std::fmt::Debug for HistoryItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HistoryItem([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreParams {
    pub id: String,
    pub commit: String,
}
impl std::fmt::Debug for RestoreParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RestoreParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashParams {
    pub profile_id: Option<String>,
}
impl std::fmt::Debug for TrashParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TrashParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashItem {
    pub id: String,
    pub profile_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub entry_type: EntryType,
    pub deleted_at: String,
    pub commit: String,
}
impl std::fmt::Debug for TrashItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TrashItem([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginData {
    pub username: String,
    pub password: Option<String>,
    pub urls: Vec<String>,
    pub totp: Option<TotpConfig>,
    pub custom: Vec<CustomField>,
}
impl std::fmt::Debug for LoginData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LoginData([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpConfig {
    pub secret: Option<String>,
    pub algorithm: TotpAlgorithm,
    pub digits: u8,
    pub period: u32,
    pub issuer: Option<String>,
}
impl std::fmt::Debug for TotpConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TotpConfig([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomField {
    pub key: String,
    pub value: Option<String>,
    pub hidden: bool,
}
impl std::fmt::Debug for CustomField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CustomField([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretData {
    pub value: Option<String>,
}
impl std::fmt::Debug for SecretData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretData([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvData {
    pub vars: Vec<EnvVar>,
}
impl std::fmt::Debug for EnvData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EnvData([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvVar {
    pub key: String,
    pub value: Option<String>,
    pub hidden: bool,
    pub comment: Option<String>,
}
impl std::fmt::Debug for EnvVar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EnvVar([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileData {
    pub filename: String,
    pub mime: String,
    pub content_base64: Option<String>,
    pub size: u64,
}
impl std::fmt::Debug for FileData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FileData([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteData {
    pub body: Option<String>,
}
impl std::fmt::Debug for NoteData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NoteData([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKeyData {
    pub private_key: Option<String>,
    pub public_key: String,
    pub passphrase: Option<String>,
    pub comment: String,
}
impl std::fmt::Debug for SshKeyData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SshKeyData([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseData {
    pub engine: String,
    pub host: String,
    pub port: Option<u16>,
    pub database: String,
    pub username: String,
    pub password: Option<String>,
    pub options: String,
}
impl std::fmt::Debug for DatabaseData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DatabaseData([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyData {
    pub key: Option<String>,
    pub secret: Option<String>,
    pub endpoint: Option<String>,
    pub expires_at: Option<String>,
}
impl std::fmt::Debug for ApiKeyData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ApiKeyData([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvResolveParams {
    pub profile_id: String,
    pub entry_ids: Option<Vec<String>>,
}
impl std::fmt::Debug for EnvResolveParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EnvResolveParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvResolveResult {
    pub vars: Vec<EnvPair>,
}
impl std::fmt::Debug for EnvResolveResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EnvResolveResult([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvPair {
    pub key: String,
    pub value: String,
}
impl std::fmt::Debug for EnvPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EnvPair([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvFormatParams {
    pub profile_id: String,
    pub entry_ids: Option<Vec<String>>,
    pub format: EnvFormat,
}
impl std::fmt::Debug for EnvFormatParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EnvFormatParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextResult {
    pub text: String,
}
impl std::fmt::Debug for TextResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TextResult([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvParseParams {
    pub text: String,
}
impl std::fmt::Debug for EnvParseParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EnvParseParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpCode {
    pub code: String,
    pub expires_in: u32,
    pub period: u32,
}
impl std::fmt::Debug for TotpCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TotpCode([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinnedTotp {
    pub id: String,
    pub profile_name: String,
    pub name: String,
}
impl std::fmt::Debug for PinnedTotp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PinnedTotp([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardCopyParams {
    pub id: String,
    pub field: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clear_after_secs: Option<u64>,
}
impl std::fmt::Debug for ClipboardCopyParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ClipboardCopyParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardCopyResult {
    pub clear_after_secs: u32,
}
impl std::fmt::Debug for ClipboardCopyResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ClipboardCopyResult([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserLaunchParams {
    pub profile_id: String,
    pub url: Option<String>,
}
impl std::fmt::Debug for BrowserLaunchParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BrowserLaunchParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserSession {
    pub profile_id: String,
    pub engine: BrowserEngine,
    pub pid: u32,
    pub started_at: String,
    pub user_data_dir: String,
}
impl std::fmt::Debug for BrowserSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BrowserSession([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedBrowser {
    pub engine: BrowserEngine,
    pub path: String,
    pub version: Option<String>,
}
impl std::fmt::Debug for DetectedBrowser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DetectedBrowser([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionArchiveInfo {
    pub exists: bool,
    pub size_bytes: u64,
    pub updated_at: Option<String>,
    pub committed: bool,
}
impl std::fmt::Debug for SessionArchiveInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionArchiveInfo([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub has_remote: bool,
    pub remote_url: Option<String>,
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
    pub dirty: bool,
    pub last_sync_at: Option<String>,
    pub conflicts: Vec<ConflictSummary>,
    pub log: Vec<CommitInfo>,
}
impl std::fmt::Debug for SyncStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SyncStatus([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitInfo {
    pub commit: String,
    pub at: String,
    pub author: String,
    pub message: String,
}
impl std::fmt::Debug for CommitInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CommitInfo([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncParams {
    pub mode: SyncMode,
}
impl std::fmt::Debug for SyncParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SyncParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub pulled: u32,
    pub pushed: u32,
    pub conflicts: Vec<ConflictSummary>,
    pub status: SyncStatus,
}
impl std::fmt::Debug for SyncResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SyncResult([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictSummary {
    pub entry_id: String,
    pub profile_id: String,
    pub name: String,
}
impl std::fmt::Debug for ConflictSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ConflictSummary([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictDetail {
    pub entry_id: String,
    pub mine: Option<Entry>,
    pub theirs: Option<Entry>,
}
impl std::fmt::Debug for ConflictDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ConflictDetail([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveParams {
    pub entry_id: String,
    pub resolution: Resolution,
    pub merged: Option<EntryInput>,
}
impl std::fmt::Debug for ResolveParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ResolveParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetRemoteParams {
    pub url: String,
}
impl std::fmt::Debug for SetRemoteParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SetRemoteParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddRecipientParams {
    pub label: String,
    pub public_key: String,
}
impl std::fmt::Debug for AddRecipientParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AddRecipientParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveRecipientParams {
    pub label: String,
}
impl std::fmt::Debug for RemoveRecipientParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RemoveRecipientParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportParams {
    pub format: ImportFormat,
    pub path: String,
    pub profile_id: Option<String>,
    pub skip_duplicates: bool,
    pub passphrase: Option<String>,
}
impl std::fmt::Debug for ImportParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ImportParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub items: Vec<ImportItem>,
    pub warnings: Vec<String>,
}
impl std::fmt::Debug for ImportPreview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ImportPreview([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportItem {
    pub name: String,
    #[serde(rename = "type")]
    pub entry_type: EntryType,
    pub profile_name: String,
    pub duplicate: bool,
}
impl std::fmt::Debug for ImportItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ImportItem([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportApplyResult {
    pub created: u32,
    pub skipped: u32,
    pub profiles_created: u32,
}
impl std::fmt::Debug for ImportApplyResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ImportApplyResult([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportParams {
    pub format: ExportFormat,
    pub path: String,
    pub profile_id: Option<String>,
    pub passphrase: Option<String>,
}
impl std::fmt::Debug for ExportParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ExportParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub path: String,
    pub entries: u32,
}
impl std::fmt::Debug for ExportResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ExportResult([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorParams {
    pub fix: bool,
    pub rebuild_manifest: bool,
    pub staged: bool,
}
impl std::fmt::Debug for DoctorParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DoctorParams([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    pub ok: bool,
    pub issues: Vec<DoctorIssue>,
    pub fixed: Vec<String>,
}
impl std::fmt::Debug for DoctorReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DoctorReport([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorIssue {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}
impl std::fmt::Debug for DoctorIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DoctorIssue([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub auto_lock_secs: u32,
    pub clipboard_clear_secs: u32,
    pub reveal_hide_secs: u32,
    pub lock_on_screen_lock: bool,
    pub tray_enabled: bool,
    pub autostart: bool,
    pub theme: Theme,
    pub global_shortcut: String,
    pub keep_browsers_open_on_lock: bool,
    pub names_in_commits: bool,
    pub auto_push: bool,
    pub sessions_commit: bool,
    pub git_name: Option<String>,
    pub git_email: Option<String>,
}
impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Settings([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum EntryPayload {
    Login(LoginData),
    Secret(SecretData),
    Env(EnvData),
    File(FileData),
    Note(NoteData),
    SshKey(SshKeyData),
    Database(DatabaseData),
    ApiKey(ApiKeyData),
}
impl std::fmt::Debug for EntryPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EntryPayload([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", content = "input", rename_all = "snake_case")]
pub enum DomainRequest {
    ValidateEntry(EntryInput),
    ValidateProfile(ProfileInput),
    ValidateRecipient(ValidateRecipientInput),
    RecipientKeyKind(String),
    ParseDotenv(String),
    FormatEnv(DomainEnvFormatInput),
    ParseOtpauth(String),
    GeneratePassword(u32),
    EstimateStrength(String),
    EntryDefaults(EntryDefaultsInput),
    DatabaseUrl(DatabaseData),
    SecretPaths(EntryPayload),
    ReadField(ReadFieldInput),
    RedactPayload(EntryPayload),
    MergeKeepOnNone(PayloadMergeInput),
    ApplyOptionalClears(EntryPayload),
    FlattenEntry(EntryInput),
    MergeEntries(MergeEntriesInput),
    DefaultBrowserSettings(()),
    FileDigest(Option<EntryInput>),
    DiffRows(DiffRowsInput),
}
impl std::fmt::Debug for DomainRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DomainRequest([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", content = "output", rename_all = "snake_case")]
pub enum DomainResponse {
    ValidateEntry(std::collections::BTreeMap<String, String>),
    ValidateProfile(std::collections::BTreeMap<String, String>),
    ValidateRecipient(std::collections::BTreeMap<String, String>),
    RecipientKeyKind(Option<DomainRecipientKeyKind>),
    ParseDotenv(Vec<EnvVar>),
    FormatEnv(String),
    ParseOtpauth(Option<ParsedOtpauth>),
    GeneratePassword(String),
    EstimateStrength(PasswordStrength),
    EntryDefaults(EntryInput),
    DatabaseUrl(String),
    SecretPaths(Vec<String>),
    ReadField(Option<String>),
    RedactPayload(RedactedPayload),
    MergeKeepOnNone(EntryPayload),
    ApplyOptionalClears(EntryPayload),
    FlattenEntry(std::collections::BTreeMap<String, String>),
    MergeEntries(EntryInput),
    DefaultBrowserSettings(BrowserSettings),
    FileDigest(Option<String>),
    DiffRows(Vec<DomainDiffRow>),
}
impl std::fmt::Debug for DomainResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DomainResponse([REDACTED])")
    }
}
