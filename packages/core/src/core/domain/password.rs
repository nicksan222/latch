use super::*;
use rand::{Rng, rngs::OsRng};

pub fn generate_password(length: u32) -> Result<String> {
    if length > 4096 {
        return Err(Error::invalid("Password length must be at most 4096"));
    }
    const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789!@#$%^&*-_=+";
    let mut rng = OsRng;
    Ok((0..length)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect())
}

pub fn estimate_strength(passphrase: String) -> Result<PasswordStrength> {
    let passphrase = zeroize::Zeroizing::new(passphrase);
    let mut hints = Vec::new();
    if passphrase.is_empty() {
        return Ok(PasswordStrength {
            score: 0,
            label: "Very weak".into(),
            bits: 0,
            hints: vec!["Enter a passphrase".into()],
        });
    }
    let words = passphrase
        .split(|c: char| c.is_whitespace() || "-_.".contains(c))
        .filter(|s| !s.is_empty())
        .count();
    let mut pool = 0;
    if passphrase.chars().any(|c| c.is_ascii_lowercase()) {
        pool += 26;
    }
    if passphrase.chars().any(|c| c.is_ascii_uppercase()) {
        pool += 26;
    }
    if passphrase.chars().any(|c| c.is_ascii_digit()) {
        pool += 10;
    }
    if passphrase.chars().any(|c| !c.is_ascii_alphanumeric()) {
        pool += 33;
    }
    let len = passphrase.encode_utf16().count();
    let mut bits = (len as f64 * (pool.max(1) as f64).log2() * 0.6).max(if words >= 3 {
        words as f64 * 11.0
    } else {
        0.0
    });
    let lower = zeroize::Zeroizing::new(passphrase.to_lowercase());
    if [
        "password", "letmein", "qwerty", "123456", "admin", "welcome", "iloveyou", "monkey",
        "dragon", "latch", "secret",
    ]
    .iter()
    .any(|w| lower.contains(w))
    {
        bits *= 0.5;
        hints.push("Avoid common words like “password”".into());
    }
    let chars = zeroize::Zeroizing::new(passphrase.chars().collect::<Vec<_>>());
    if chars
        .windows(3)
        .any(|w| w[0] == w[1] && w[1] == w[2] && w[0] != '\n' && w[0] != '\r')
    {
        bits *= 0.8;
        hints.push("Avoid repeated characters".into());
    }
    if [
        "0123", "1234", "2345", "3456", "4567", "5678", "6789", "abcd",
    ]
    .iter()
    .any(|s| lower.starts_with(s))
    {
        bits *= 0.7;
    }
    if len < 12 {
        hints.push("Use at least 12 characters".into());
    }
    if words < 4 && len < 20 {
        hints.push("Four or more random words work well".into());
    }
    let score = if bits < 28.0 {
        0
    } else if bits < 40.0 {
        1
    } else if bits < 55.0 {
        2
    } else if bits < 70.0 {
        3
    } else {
        4
    };
    Ok(PasswordStrength {
        score,
        label: ["Very weak", "Weak", "Fair", "Strong", "Very strong"][score as usize].into(),
        bits: bits.round() as u32,
        hints,
    })
}
