//! Generates transitional serde DTOs solely from the compiled protobuf descriptor.
use std::fmt::Write;

use prost_types::{
    DescriptorProto, EnumDescriptorProto, FieldDescriptorProto, FileDescriptorProto,
    field_descriptor_proto::{Label, Type},
};

fn snake(name: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for c in name.chars() {
        if c.is_uppercase() && prev_lower {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
        prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
    }
    out
}

fn pascal(name: &str) -> String {
    name.split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|c| c.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase())
                .unwrap_or_default()
        })
        .collect()
}

fn comments(file: &FileDescriptorProto, path: &[i32]) -> String {
    file.source_code_info
        .as_ref()
        .and_then(|info| info.location.iter().find(|location| location.path == path))
        .map(|location| {
            format!(
                "{} {}",
                location.leading_comments.as_deref().unwrap_or(""),
                location.trailing_comments.as_deref().unwrap_or("")
            )
        })
        .unwrap_or_default()
}

fn annotation(comments: &str, key: &str) -> Option<String> {
    comments
        .split(key)
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .map(str::to_owned)
}

fn type_name(field: &FieldDescriptorProto) -> String {
    match field.r#type() {
        Type::String => "String".into(),
        Type::Bool => "bool".into(),
        Type::Uint32 => "u32".into(),
        Type::Uint64 => "u64".into(),
        Type::Message | Type::Enum => field
            .type_name
            .as_deref()
            .and_then(|name| name.strip_prefix(".latch.v1."))
            .unwrap_or("Unknown")
            .to_owned(),
        other => panic!("unsupported compatibility field type {other:?}"),
    }
}

fn field_type(field: &FieldDescriptorProto, comments: &str) -> String {
    let mut base = type_name(field);
    if base == "OptionalStringList" {
        return "Option<Vec<String>>".into();
    }
    if let Some(bits) = annotation(comments, "legacy_uint_bits:") {
        base = match bits.as_str() {
            "8" => "u8",
            "16" => "u16",
            _ => panic!("unsupported legacy width"),
        }
        .into();
    }
    if field.label() == Label::Repeated {
        return format!("Vec<{base}>");
    }
    if field.proto3_optional.unwrap_or(false)
        || annotation(comments, "legacy_optional:").as_deref() == Some("true")
    {
        return format!("Option<{base}>");
    }
    base
}

fn helper_type(proto: &str) -> String {
    match proto {
        "Empty" => "()".into(),
        "StringValue" => "String".into(),
        "UInt32Value" => "u32".into(),
        "OptionalString" => "Option<String>".into(),
        "OptionalEntryInput" => "Option<EntryInput>".into(),
        "OptionalDomainRecipientKeyKind" => "Option<DomainRecipientKeyKind>".into(),
        "OptionalParsedOtpauth" => "Option<ParsedOtpauth>".into(),
        "StringMap" => "std::collections::BTreeMap<String, String>".into(),
        "DomainEnvVarList" => "Vec<EnvVar>".into(),
        "DomainStringList" => "Vec<String>".into(),
        "DomainDiffRowList" => "Vec<DomainDiffRow>".into(),
        other => other.to_owned(),
    }
}

fn union(out: &mut String, message: &DescriptorProto) {
    let name = message.name.as_deref().expect("named union");
    let (tag, content) = match name {
        "EntryPayload" => ("type", "data"),
        "DomainRequest" => ("operation", "input"),
        "DomainResponse" => ("operation", "output"),
        _ => unreachable!(),
    };
    writeln!(out, "#[derive(Clone, PartialEq, Serialize, Deserialize)]").unwrap();
    writeln!(
        out,
        "#[serde(tag = {tag:?}, content = {content:?}, rename_all = \"snake_case\")]"
    )
    .unwrap();
    writeln!(out, "pub enum {name} {{").unwrap();
    for field in &message.field {
        let variant = pascal(field.name.as_deref().expect("union variant"));
        let ty = helper_type(&type_name(field));
        writeln!(out, "    {variant}({ty}),").unwrap();
    }
    writeln!(out, "}}").unwrap();
    writeln!(out, "impl std::fmt::Debug for {name} {{ fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{ f.write_str(\"{name}([REDACTED])\") }} }}").unwrap();
}

fn regular_message(
    out: &mut String,
    file: &FileDescriptorProto,
    message: &DescriptorProto,
    message_index: usize,
) {
    let name = message.name.as_deref().expect("named message");
    writeln!(out, "#[derive(Clone, PartialEq, Serialize, Deserialize)]").unwrap();
    writeln!(out, "#[serde(rename_all = \"camelCase\")]").unwrap();
    writeln!(out, "pub struct {name} {{").unwrap();
    for (field_index, field) in message.field.iter().enumerate() {
        let note = comments(file, &[4, message_index as i32, 2, field_index as i32]);
        let proto_name = field.name.as_deref().expect("named field");
        let rust_name =
            annotation(&note, "legacy_rust_name:").unwrap_or_else(|| proto_name.to_owned());
        let rust_name = if rust_name == "type" {
            "r#type".into()
        } else {
            rust_name
        };
        if annotation(&note, "legacy_omittable:").as_deref() == Some("true") {
            writeln!(
                out,
                "    #[serde(default, skip_serializing_if = \"Option::is_none\")]"
            )
            .unwrap();
        } else if annotation(&note, "legacy_default:").as_deref() == Some("true") {
            writeln!(out, "    #[serde(default)]").unwrap();
        }
        if rust_name != proto_name && proto_name == "type" {
            writeln!(out, "    #[serde(rename = \"type\")]").unwrap();
        }
        writeln!(out, "    pub {rust_name}: {},", field_type(field, &note)).unwrap();
    }
    writeln!(out, "}}").unwrap();
    writeln!(out, "impl std::fmt::Debug for {name} {{ fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{ f.write_str(\"{name}([REDACTED])\") }} }}").unwrap();
}

fn rust_enum(
    out: &mut String,
    file: &FileDescriptorProto,
    enumeration: &EnumDescriptorProto,
    enum_index: usize,
) {
    let name = enumeration.name.as_deref().expect("named enum");
    let prefix = format!("{}_", snake(name).to_ascii_uppercase());
    writeln!(
        out,
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]"
    )
    .unwrap();
    writeln!(out, "#[serde(rename_all = \"snake_case\")]").unwrap();
    writeln!(out, "pub enum {name} {{").unwrap();
    for (value_index, value) in enumeration.value.iter().enumerate().skip(1) {
        let proto_name = value.name.as_deref().expect("enum value");
        let suffix = proto_name.strip_prefix(&prefix).expect("enum prefix");
        let variant = pascal(suffix);
        let note = comments(file, &[5, enum_index as i32, 2, value_index as i32]);
        if let Some(legacy) = annotation(&note, "legacy:")
            && legacy != snake(&variant)
        {
            writeln!(out, "    #[serde(rename = {legacy:?})]").unwrap();
        }
        writeln!(out, "    {variant},").unwrap();
    }
    writeln!(out, "}}").unwrap();
}

pub fn generate(file: &FileDescriptorProto) -> String {
    let mut out = String::from(
        "// Generated from proto/latch/v1/agent.proto. Do not edit.\nuse serde::{Serialize, Deserialize};\n",
    );
    for (index, enumeration) in file.enum_type.iter().enumerate() {
        rust_enum(&mut out, file, enumeration, index);
    }
    for (index, message) in file.message_type.iter().enumerate() {
        let name = message.name.as_deref().expect("named message");
        if matches!(
            name,
            "Empty"
                | "StringValue"
                | "UInt32Value"
                | "OptionalString"
                | "OptionalEntryInput"
                | "OptionalDomainRecipientKeyKind"
                | "OptionalParsedOtpauth"
                | "StringMap"
                | "DomainEnvVarList"
                | "DomainStringList"
                | "DomainDiffRowList"
                | "OptionalStringList"
        ) {
            continue;
        }
        if name.ends_with("Response")
            && message.field.len() == 1
            && message.field[0].name.as_deref() == Some("items")
        {
            continue;
        }
        if matches!(name, "EntryPayload" | "DomainRequest" | "DomainResponse") {
            union(&mut out, message);
        } else {
            regular_message(&mut out, file, message, index);
        }
    }
    out
}
