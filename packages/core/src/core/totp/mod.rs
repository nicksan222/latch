//! RFC 6238 TOTP (HMAC-SHA1/256/512, 6 or 8 digits) and `otpauth://` URIs.

use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, Mac};
use zeroize::Zeroizing;

use super::error::{Error, Result};
use super::format::TotpDoc;
use crate::protocol::{TotpAlgorithm, TotpCode};

/// Normalize a base32 secret: strip spaces/dashes/padding, uppercase. Errors
/// (without echoing the secret) when it is not valid base32.
pub fn normalize_secret(s: &str) -> Result<String> {
    let n: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-' && *c != '=')
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if n.is_empty() {
        return Err(Error::invalid("TOTP secret is empty"));
    }
    decode_secret(&n)?;
    Ok(n)
}

fn decode_secret(s: &str) -> Result<Zeroizing<Vec<u8>>> {
    BASE32_NOPAD
        .decode(s.as_bytes())
        .map(Zeroizing::new)
        .map_err(|_| Error::invalid("TOTP secret is not valid base32"))
}

fn hotp(key: &[u8], counter: u64, alg: TotpAlgorithm, digits: u8) -> String {
    let msg = counter.to_be_bytes();
    let mac: Zeroizing<Vec<u8>> = Zeroizing::new(match alg {
        TotpAlgorithm::Sha1 => {
            let mut m = Hmac::<sha1::Sha1>::new_from_slice(key).expect("any key length");
            m.update(&msg);
            m.finalize().into_bytes().to_vec()
        }
        TotpAlgorithm::Sha256 => {
            let mut m = Hmac::<sha2::Sha256>::new_from_slice(key).expect("any key length");
            m.update(&msg);
            m.finalize().into_bytes().to_vec()
        }
        TotpAlgorithm::Sha512 => {
            let mut m = Hmac::<sha2::Sha512>::new_from_slice(key).expect("any key length");
            m.update(&msg);
            m.finalize().into_bytes().to_vec()
        }
    });
    let off = (mac[mac.len() - 1] & 0x0f) as usize;
    let bin = ((u32::from(mac[off]) & 0x7f) << 24)
        | (u32::from(mac[off + 1]) << 16)
        | (u32::from(mac[off + 2]) << 8)
        | u32::from(mac[off + 3]);
    let m = 10u32.pow(u32::from(digits));
    format!("{:0width$}", bin % m, width = digits as usize)
}

/// TOTP code for raw key bytes at a unix time.
pub fn totp_raw(key: &[u8], unix: u64, period: u32, alg: TotpAlgorithm, digits: u8) -> String {
    hotp(key, unix / u64::from(period.max(1)), alg, digits)
}

fn check_params(digits: u8, period: u32) -> Result<()> {
    if digits != 6 && digits != 8 {
        return Err(Error::invalid("TOTP digits must be 6 or 8"));
    }
    if period == 0 || period > 86_400 {
        return Err(Error::invalid("TOTP period must be 1..86400 seconds"));
    }
    Ok(())
}

pub fn validate(t: &TotpDoc) -> Result<()> {
    check_params(t.digits, t.period)?;
    decode_secret(&t.secret)?;
    Ok(())
}

/// Code at a given unix time.
pub fn code_at(t: &TotpDoc, unix: u64) -> Result<TotpCode> {
    validate(t)?;
    let key = decode_secret(&t.secret)?;
    let code = totp_raw(&key, unix, t.period, t.algorithm, t.digits);
    let expires_in = t.period - (unix % u64::from(t.period)) as u32;
    Ok(TotpCode {
        code,
        expires_in,
        period: t.period,
    })
}

pub fn code_now(t: &TotpDoc) -> Result<TotpCode> {
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    code_at(t, unix)
}

/// Parse `otpauth://totp/Label?secret=…&issuer=…&algorithm=…&digits=…&period=…`.
pub fn parse_otpauth(uri: &str) -> Result<TotpDoc> {
    let u = url::Url::parse(uri.trim()).map_err(|_| Error::invalid("malformed otpauth URI"))?;
    if u.scheme() != "otpauth" {
        return Err(Error::invalid("not an otpauth URI"));
    }
    if u.host_str() != Some("totp") {
        return Err(Error::invalid("only otpauth://totp URIs are supported"));
    }
    let mut secret = None;
    let mut issuer = None;
    let mut algorithm = TotpAlgorithm::Sha1;
    let mut digits = 6u8;
    let mut period = 30u32;
    for (k, v) in u.query_pairs() {
        match k.to_ascii_lowercase().as_str() {
            "secret" => secret = Some(Zeroizing::new(v.into_owned())),
            "issuer" => issuer = Some(v.into_owned()),
            "algorithm" => {
                algorithm = match v.to_ascii_uppercase().as_str() {
                    "SHA1" => TotpAlgorithm::Sha1,
                    "SHA256" => TotpAlgorithm::Sha256,
                    "SHA512" => TotpAlgorithm::Sha512,
                    _ => return Err(Error::invalid("unsupported TOTP algorithm")),
                }
            }
            "digits" => {
                digits = v
                    .parse()
                    .map_err(|_| Error::invalid("invalid TOTP digits"))?
            }
            "period" => {
                period = v
                    .parse()
                    .map_err(|_| Error::invalid("invalid TOTP period"))?
            }
            _ => {}
        }
    }
    if issuer.is_none() {
        let label = percent_decode(u.path().trim_start_matches('/'));
        if let Some((iss, _)) = label.split_once(':')
            && !iss.trim().is_empty()
        {
            issuer = Some(iss.trim().to_owned());
        }
    }
    let secret = secret.ok_or_else(|| Error::invalid("otpauth URI has no secret"))?;
    let t = TotpDoc {
        secret: normalize_secret(&secret)?,
        algorithm,
        digits,
        period,
        issuer,
    };
    check_params(t.digits, t.period)?;
    Ok(t)
}

pub(crate) fn percent_decode(s: &str) -> String {
    // A URI path is not form data: literal plus signs stay plus signs.
    percent_encoding::percent_decode_str(s)
        .decode_utf8_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 6238 Appendix B.
    const SEED1: &[u8] = b"12345678901234567890";
    const SEED256: &[u8] = b"12345678901234567890123456789012";
    const SEED512: &[u8] = b"1234567890123456789012345678901234567890123456789012345678901234";

    #[test]
    fn rfc6238_vectors() {
        let cases: &[(u64, &str, &str, &str)] = &[
            (59, "94287082", "46119246", "90693936"),
            (1111111109, "07081804", "68084774", "25091201"),
            (1111111111, "14050471", "67062674", "99943326"),
            (1234567890, "89005924", "91819424", "93441116"),
            (2000000000, "69279037", "90698825", "38618901"),
            (20000000000, "65353130", "77737706", "47863826"),
        ];
        for (t, s1, s256, s512) in cases {
            assert_eq!(
                totp_raw(SEED1, *t, 30, TotpAlgorithm::Sha1, 8),
                *s1,
                "sha1 {t}"
            );
            assert_eq!(
                totp_raw(SEED256, *t, 30, TotpAlgorithm::Sha256, 8),
                *s256,
                "sha256 {t}"
            );
            assert_eq!(
                totp_raw(SEED512, *t, 30, TotpAlgorithm::Sha512, 8),
                *s512,
                "sha512 {t}"
            );
            // 6 digits is the low 6 digits of the same truncation.
            assert_eq!(totp_raw(SEED1, *t, 30, TotpAlgorithm::Sha1, 6), s1[2..]);
        }
    }

    #[test]
    fn code_from_doc() {
        let t = TotpDoc {
            secret: BASE32_NOPAD.encode(SEED1),
            algorithm: TotpAlgorithm::Sha1,
            digits: 8,
            period: 30,
            issuer: None,
        };
        let c = code_at(&t, 59).unwrap();
        assert_eq!(c.code, "94287082");
        assert_eq!(c.expires_in, 1);
        assert_eq!(c.period, 30);
        assert_eq!(code_at(&t, 60).unwrap().expires_in, 30);
        assert_eq!(code_now(&t).unwrap().code.len(), 8);
        let mut bad = t.clone();
        bad.digits = 7;
        assert!(code_at(&bad, 0).is_err());
    }

    #[test]
    fn short_base32_secrets_remain_supported() {
        // Independent Python stdlib HMAC fixtures at unix=59, counter=1.
        // Both keys are shorter than the RFC minimum enforced by some builders.
        for (secret, expected) in [
            ("MY", ["300412", "114700", "767561"]),
            ("JBSWY3DPEHPK3PXP", ["996554", "344551", "439887"]),
        ] {
            for (algorithm, expected) in [
                TotpAlgorithm::Sha1,
                TotpAlgorithm::Sha256,
                TotpAlgorithm::Sha512,
            ]
            .into_iter()
            .zip(expected)
            {
                let mut t = parse_otpauth(&format!("otpauth://totp/test?secret={secret}")).unwrap();
                t.algorithm = algorithm;
                assert!(validate(&t).is_ok());
                let code = code_at(&t, 59).unwrap();
                assert_eq!(code.code, expected);
                assert_eq!(code.expires_in, 1);
            }
        }
    }

    #[test]
    fn invalid_generation_parameters_are_rejected_without_secret_disclosure() {
        let t = TotpDoc {
            secret: "JBSWY3DPEHPK3PXP".into(),
            algorithm: TotpAlgorithm::Sha1,
            digits: 6,
            period: 30,
            issuer: None,
        };
        for (digits, period) in [(0, 30), (7, 30), (10, 30), (255, 30), (6, 0), (6, 86_401)] {
            let mut invalid = t.clone();
            invalid.digits = digits;
            invalid.period = period;
            for error in [
                validate(&invalid).unwrap_err(),
                code_at(&invalid, 59).unwrap_err(),
            ] {
                assert!(!error.to_string().contains(&t.secret));
            }
        }
        for secret in ["not!base32", "MZ"] {
            let mut invalid = t.clone();
            invalid.secret = secret.into();
            let error = code_at(&invalid, 59).unwrap_err().to_string();
            assert!(!error.contains(secret));
            assert!(validate(&invalid).is_err());
        }
        for period in [1, 86_400] {
            let mut boundary = t.clone();
            boundary.period = period;
            assert_eq!(code_at(&boundary, 0).unwrap().expires_in, period);
        }
    }

    #[test]
    fn normalize() {
        assert_eq!(
            normalize_secret("jbsw y3dp ehpk 3pxp").unwrap(),
            "JBSWY3DPEHPK3PXP"
        );
        assert_eq!(
            normalize_secret("JBSWY3DPEHPK3PXP====").unwrap(),
            "JBSWY3DPEHPK3PXP"
        );
        let e = normalize_secret("not!base32").unwrap_err();
        assert!(!e.to_string().contains("not!base32"));
        assert!(normalize_secret("").is_err());
    }

    #[test]
    fn otpauth_label_decoding_preserves_path_characters() {
        for label in [
            "ACME&Co+Ops:alice&bob+tag@example.com",
            "ACME%26Co%2BOps:alice%26bob%2Btag%40example.com",
        ] {
            let uri = format!("otpauth://totp/{label}?secret=JBSWY3DPEHPK3PXP");
            let parsed = parse_otpauth(&uri).unwrap();
            assert_eq!(parsed.issuer.as_deref(), Some("ACME&Co+Ops"));
            assert_eq!(
                percent_decode(label),
                "ACME&Co+Ops:alice&bob+tag@example.com"
            );
        }
    }

    #[test]
    fn otpauth() {
        let t = parse_otpauth(
            "otpauth://totp/ACME%20Co:john@example.com?secret=JBSWY3DPEHPK3PXP&issuer=ACME%20Co&algorithm=SHA256&digits=8&period=60",
        )
        .unwrap();
        assert_eq!(t.secret, "JBSWY3DPEHPK3PXP");
        assert_eq!(t.algorithm, TotpAlgorithm::Sha256);
        assert_eq!(t.digits, 8);
        assert_eq!(t.period, 60);
        assert_eq!(t.issuer.as_deref(), Some("ACME Co"));

        let t = parse_otpauth("otpauth://totp/GitHub:me?secret=JBSWY3DPEHPK3PXP").unwrap();
        assert_eq!(t.issuer.as_deref(), Some("GitHub"));
        assert_eq!(
            (t.digits, t.period, t.algorithm),
            (6, 30, TotpAlgorithm::Sha1)
        );

        assert!(parse_otpauth("otpauth://hotp/x?secret=JBSWY3DPEHPK3PXP").is_err());
        assert!(parse_otpauth("https://x").is_err());
        assert!(parse_otpauth("otpauth://totp/x").is_err());
        assert!(parse_otpauth("otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&digits=7").is_err());
    }
}
