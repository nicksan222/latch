//! Random identifiers: 16 bytes from the OS CSPRNG, base32 lowercase without
//! padding (26 characters). Ids are never derived from names.

use data_encoding::BASE32_NOPAD;

pub const ID_LEN: usize = 26;

/// Generate a new random id.
pub fn new_id() -> String {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).expect("OS random number generator unavailable");
    BASE32_NOPAD.encode(&bytes).to_ascii_lowercase()
}

/// True when `s` has the shape of a Latch id (safe to use as a path segment).
pub fn is_valid_id(s: &str) -> bool {
    s.len() == ID_LEN
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b))
        && BASE32_NOPAD
            .decode(s.to_ascii_uppercase().as_bytes())
            .is_ok_and(|v| v.len() == 16)
}

/// Validate an id coming from a caller; returns InvalidParams otherwise.
pub fn check_id(s: &str) -> crate::core::Result<()> {
    if is_valid_id(s) {
        Ok(())
    } else {
        Err(crate::core::Error::invalid(format!("malformed id `{s}`")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape() {
        for _ in 0..100 {
            let id = new_id();
            assert_eq!(id.len(), ID_LEN);
            assert!(is_valid_id(&id), "{id}");
            assert_eq!(id, id.to_ascii_lowercase());
        }
    }

    #[test]
    fn unique() {
        let a = new_id();
        let b = new_id();
        assert_ne!(a, b);
    }

    #[test]
    fn rejects_bad() {
        assert!(!is_valid_id(""));
        assert!(!is_valid_id("../../etc/passwd"));
        assert!(!is_valid_id(&"a".repeat(25)));
        assert!(!is_valid_id(&"1".repeat(26)));
        assert!(!is_valid_id(&new_id().to_ascii_uppercase()));
        assert!(check_id("nope").is_err());
    }
}
