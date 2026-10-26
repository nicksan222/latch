//! Env secret strategy.

use super::super::*;

secret_kind! {
    id: "env",
    ui: (2, "Env", None),
    defaults: serde_json::json!({"vars": []}),
    wire_secrets: &[("vars.*", false)],
    variant: Env, data: EnvData, doc: EnvDoc,
    diff_fields: &["vars.*"],
    validate_ui: validate_ui,
    merge: from_input,
    payload: to_payload,
    paths: secret_paths,
    reveal: reveal_field,
    subtitle: subtitle,
    digests: field_digests,
}

fn from_input(d: &EnvData, s: Option<&EnvDoc>) -> Result<EnvDoc> {
    unique_keys(d.vars.iter().map(|v| v.key.as_str()), "env var")?;
    for v in &d.vars {
        if !crate::core::environment::is_valid_key(&v.key) {
            return Err(Error::invalid(format!("invalid env var name `{}`", v.key)));
        }
    }
    Ok(EnvDoc {
        vars: d
            .vars
            .iter()
            .map(|v| EnvVarDoc {
                key: v.key.clone(),
                value: keep(
                    &v.value,
                    s.and_then(|s| s.vars.iter().find(|x| x.key == v.key))
                        .map(|x| &x.value),
                ),
                hidden: v.hidden,
                comment: v.comment.clone().filter(|c| !c.is_empty()),
            })
            .collect(),
    })
}

fn to_payload(d: &EnvDoc, reveal: bool) -> EnvData {
    EnvData {
        vars: d
            .vars
            .iter()
            .map(|v| EnvVar {
                key: v.key.clone(),
                value: opt(reveal || !v.hidden, &v.value),
                hidden: v.hidden,
                comment: v.comment.clone(),
            })
            .collect(),
    }
}

fn secret_paths(d: &EnvDoc) -> Vec<String> {
    let mut v = Vec::new();
    v.extend(
        d.vars
            .iter()
            .filter(|x| x.hidden)
            .map(|x| format!("vars.{}", x.key)),
    );
    v
}

fn reveal_field(d: &EnvDoc, field: &str) -> Result<Zeroizing<String>> {
    let nf = || Error::NotFound(format!("field `{field}`"));
    let z = |s: &str| Ok(Zeroizing::new(s.to_owned()));
    match field.strip_prefix("vars.") {
        Some(k) => d
            .vars
            .iter()
            .find(|v| v.key == k)
            .map_or_else(|| Err(nf()), |v| z(&v.value)),
        None => Err(nf()),
    }
}

fn subtitle(d: &EnvDoc) -> String {
    let n = d.vars.len();
    format!("{n} var{}", if n == 1 { "" } else { "s" })
}

fn field_digests(d: &EnvDoc, put: &mut dyn FnMut(&str, [u8; 32])) {
    for v in &d.vars {
        put(
            &format!("vars.{}", v.key),
            digest(&(&v.value, v.hidden, &v.comment)),
        );
    }
}

fn validate_ui(d: &EnvData, errors: &mut BTreeMap<String, String>) {
    #[allow(unused_imports)]
    use crate::core::domain::validation::{issue, keys, urlish};
    keys(d.vars.iter().map(|v| v.key.as_str()), "vars", true, errors);
}
