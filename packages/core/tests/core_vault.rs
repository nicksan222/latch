// Cargo discovery only; acceptance tests live with their behavior owners.

include!("../src/core/vault/tests/core_vault_support.rs");
include!("../src/core/profiles/tests/core_vault.rs");
include!("../src/core/secrets/tests/core_vault.rs");
include!("../src/core/environment/tests/core_vault.rs");
include!("../src/core/totp/tests/core_vault.rs");
include!("../src/core/git/tests/core_vault.rs");
include!("../src/core/vault/tests/core_vault.rs");
