//! Descriptor-backed bridge for the transitional serde DTOs. Protobuf is the
//! wire format; JSON values exist only inside the old application adapters.

use std::sync::OnceLock;

use prost::Message;
use prost_reflect::{DescriptorPool, DynamicMessage, Kind, MessageDescriptor, SerializeOptions};
use serde_json::{Value, json};

use crate::{DESCRIPTOR_SET, types, v1};

include!(concat!(env!("OUT_DIR"), "/latch_narrow.rs"));
include!(concat!(env!("OUT_DIR"), "/latch_omit.rs"));

fn error(code: types::ErrorCode, message: &'static str) -> types::RpcError {
    types::RpcError {
        code,
        message: message.into(),
    }
}

fn pool() -> &'static DescriptorPool {
    static POOL: OnceLock<DescriptorPool> = OnceLock::new();
    POOL.get_or_init(|| DescriptorPool::decode(DESCRIPTOR_SET).expect("compiled descriptor set"))
}

pub fn descriptor(full_name: &str) -> Result<MessageDescriptor, types::RpcError> {
    pool()
        .get_message_by_name(full_name)
        .ok_or_else(|| error(types::ErrorCode::Internal, "unknown RPC message type"))
}

fn enum_legacy(name: &str) -> String {
    if name.starts_with("TOTP_ALGORITHM_SHA") {
        return name.rsplit('_').next().unwrap_or(name).to_owned();
    }
    let (_, suffix) = name.split_once('_').unwrap_or(("", name));
    // The type prefix may contain underscores. The caller strips it using
    // the descriptor's enum name; this fallback is for unknown future values.
    suffix.to_ascii_lowercase()
}

fn enum_value(value: &mut Value, desc: &prost_reflect::EnumDescriptor, outgoing: bool) {
    let Some(name) = value.as_str() else { return };
    let prefix = desc
        .name()
        .chars()
        .enumerate()
        .fold(String::new(), |mut out, (i, c)| {
            if c.is_uppercase() && i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_uppercase());
            out
        });
    if outgoing {
        let stripped = name.strip_prefix(&(prefix + "_")).unwrap_or(name);
        *value = Value::String(if desc.name() == "TotpAlgorithm" {
            stripped.to_owned()
        } else {
            stripped.to_ascii_lowercase()
        });
    } else if let Some(found) = desc.values().find(|candidate| {
        let stripped = candidate
            .name()
            .strip_prefix(&(prefix.clone() + "_"))
            .unwrap_or(candidate.name());
        stripped.eq_ignore_ascii_case(name) || enum_legacy(candidate.name()) == name
    }) {
        *value = Value::String(found.name().to_owned());
    }
}

fn normalize(value: &mut Value, desc: &MessageDescriptor, outgoing: bool) {
    let wrapper = matches!(
        desc.name(),
        "StringValue"
            | "UInt32Value"
            | "OptionalString"
            | "OptionalEntryInput"
            | "OptionalDomainRecipientKeyKind"
            | "OptionalParsedOtpauth"
    );
    let list_wrapper = matches!(
        desc.name(),
        "StringMap"
            | "DomainEnvVarList"
            | "DomainStringList"
            | "DomainDiffRowList"
            | "OptionalStringList"
    );
    if !outgoing && (wrapper || list_wrapper) {
        let key = if wrapper { "value" } else { "items" };
        let mut object = serde_json::Map::new();
        if !value.is_null() {
            object.insert(key.into(), value.take());
        }
        *value = Value::Object(object);
    }
    if !outgoing && desc.name() == "Empty" && value.is_null() {
        *value = json!({});
    }
    if !outgoing
        && matches!(
            desc.name(),
            "EntryPayload" | "DomainRequest" | "DomainResponse"
        )
        && let Some(object) = value.as_object_mut()
    {
        let kind = if desc.name() == "EntryPayload" {
            "type"
        } else {
            "operation"
        };
        let content = if desc.name() == "DomainResponse" {
            "output"
        } else if desc.name() == "DomainRequest" {
            "input"
        } else {
            "data"
        };
        if let (Some(Value::String(tag)), Some(data)) =
            (object.get(kind).cloned(), object.get(content).cloned())
        {
            let key = tag
                .split('_')
                .enumerate()
                .map(|(i, p)| {
                    if i == 0 {
                        p.to_owned()
                    } else {
                        let mut chars = p.chars();
                        chars
                            .next()
                            .map(|c| c.to_ascii_uppercase().to_string() + chars.as_str())
                            .unwrap_or_default()
                    }
                })
                .collect::<String>();
            object.clear();
            object.insert(key, data);
        }
    }
    let Some(object) = value.as_object_mut() else {
        return;
    };
    for field in desc.fields() {
        let key = field.json_name();
        if outgoing
            && legacy_omittable(desc.full_name(), key)
            && object.get(key).is_none_or(Value::is_null)
        {
            object.remove(key);
            continue;
        }
        if outgoing
            && !object.contains_key(key)
            && field.supports_presence()
            && field
                .containing_oneof()
                .is_none_or(|oneof| oneof.is_synthetic())
        {
            object.insert(key.into(), Value::Null);
        }
        let Some(item) = object.get_mut(key) else {
            continue;
        };
        if item.is_null() {
            if !outgoing
                && field
                    .containing_oneof()
                    .is_some_and(|oneof| !oneof.is_synthetic())
                && matches!(field.kind(), Kind::Message(_))
            {
                *item = json!({});
                continue;
            } else {
                continue;
            }
        }
        if field.is_list() {
            if let Some(array) = item.as_array_mut() {
                for child in array {
                    normalize_kind(child, field.kind(), outgoing);
                }
            }
        } else if field.is_map() {
            if let Some(map) = item.as_object_mut()
                && let Kind::Message(message) = field.kind()
                && let Some(value_field) = message.get_field_by_name("value")
            {
                for child in map.values_mut() {
                    normalize_kind(child, value_field.kind(), outgoing);
                }
            }
        } else {
            normalize_kind(item, field.kind(), outgoing);
        }
    }
    if outgoing && (wrapper || list_wrapper) {
        let key = if wrapper { "value" } else { "items" };
        *value = value.get(key).cloned().unwrap_or(Value::Null);
    }
    if outgoing && desc.name() == "Empty" {
        *value = Value::Null;
    }
    if outgoing
        && matches!(
            desc.name(),
            "EntryPayload" | "DomainRequest" | "DomainResponse"
        )
        && let Some(object) = value.as_object_mut()
        && let Some((key, data)) = object.iter().next().map(|(k, v)| (k.clone(), v.clone()))
    {
        let kind = if desc.name() == "EntryPayload" {
            "type"
        } else {
            "operation"
        };
        let content = if desc.name() == "DomainResponse" {
            "output"
        } else if desc.name() == "DomainRequest" {
            "input"
        } else {
            "data"
        };
        object.clear();
        object.insert(
            kind.into(),
            Value::String(
                key.chars()
                    .flat_map(|c| {
                        if c.is_uppercase() {
                            vec!['_', c.to_ascii_lowercase()]
                        } else {
                            vec![c]
                        }
                    })
                    .collect::<String>()
                    .trim_start_matches('_')
                    .into(),
            ),
        );
        object.insert(content.into(), data);
    }
}

fn normalize_kind(value: &mut Value, kind: Kind, outgoing: bool) {
    match kind {
        Kind::Message(desc) => normalize(value, &desc, outgoing),
        Kind::Enum(desc) => enum_value(value, &desc, outgoing),
        _ => {}
    }
}

fn validate_narrow(value: &Value, desc: &MessageDescriptor) -> Result<(), types::RpcError> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    for field in desc.fields() {
        let Some(item) = object.get(field.json_name()) else {
            continue;
        };
        if let Some(max) = narrow_bound(desc.full_name(), field.json_name()) {
            let number = item
                .as_u64()
                .or_else(|| item.as_str().and_then(|text| text.parse::<u64>().ok()));
            if number.is_some_and(|number| number > max) {
                return Err(error(
                    types::ErrorCode::InvalidParams,
                    "RPC integer outside legacy range",
                ));
            }
        }
        if let Kind::Message(child) = field.kind() {
            if field.is_list() {
                if let Some(items) = item.as_array() {
                    for item in items {
                        validate_narrow(item, &child)?;
                    }
                }
            } else if !field.is_map() {
                validate_narrow(item, &child)?;
            }
        }
    }
    Ok(())
}

/// Decode a concrete prost message into the preexisting application's JSON shape.
pub fn to_legacy<M: Message>(message: &M, full_name: &str) -> Result<Value, types::RpcError> {
    let desc = descriptor(full_name)?;
    if desc.name() == "Empty" {
        return Ok(Value::Null);
    }
    let dynamic = DynamicMessage::decode(desc.clone(), message.encode_to_vec().as_slice())
        .map_err(|_| error(types::ErrorCode::Internal, "invalid RPC response"))?;
    let mut value = dynamic
        .serialize_with_options(
            serde_json::value::Serializer,
            &SerializeOptions::new()
                .stringify_64_bit_integers(false)
                .skip_default_fields(false),
        )
        .map_err(|_| error(types::ErrorCode::Internal, "invalid RPC response"))?;
    normalize(&mut value, &desc, true);
    if desc.name().ends_with("Response") && desc.get_field_by_name("items").is_some() {
        return Ok(value.get("items").cloned().unwrap_or_else(|| json!([])));
    }
    Ok(value)
}

/// Encode an application JSON DTO into a concrete prost message.
pub fn from_legacy<M: Message + Default>(
    mut value: Value,
    full_name: &str,
) -> Result<M, types::RpcError> {
    let desc = descriptor(full_name)?;
    if desc.name() == "Empty" {
        return Ok(M::default());
    }
    if desc.name().ends_with("Response")
        && desc.get_field_by_name("items").is_some()
        && value.is_array()
    {
        value = json!({ "items": value });
    }
    normalize(&mut value, &desc, false);
    validate_narrow(&value, &desc)?;
    let dynamic = DynamicMessage::deserialize(desc, value)
        .map_err(|_| error(types::ErrorCode::InvalidParams, "invalid RPC parameters"))?;
    M::decode(dynamic.encode_to_vec().as_slice())
        .map_err(|_| error(types::ErrorCode::InvalidParams, "invalid RPC parameters"))
}

/// Preserve the core's stable error code and safe message in gRPC status details.
pub fn status(error: &types::RpcError) -> tonic::Status {
    let code = match error.code {
        types::ErrorCode::Locked => 1,
        types::ErrorCode::NoVault => 2,
        types::ErrorCode::NotFound => 3,
        types::ErrorCode::AlreadyExists => 4,
        types::ErrorCode::InvalidParams => 5,
        types::ErrorCode::BadCredentials => 6,
        types::ErrorCode::Integrity => 7,
        types::ErrorCode::Conflict => 8,
        types::ErrorCode::Denied => 9,
        types::ErrorCode::UnsupportedVersion => 10,
        types::ErrorCode::Git => 11,
        types::ErrorCode::Io => 12,
        types::ErrorCode::Internal => 13,
    };
    let detail = v1::RpcError {
        code,
        message: error.message.clone(),
    };
    let grpc_code = match error.code {
        types::ErrorCode::Locked
        | types::ErrorCode::NoVault
        | types::ErrorCode::UnsupportedVersion => tonic::Code::FailedPrecondition,
        types::ErrorCode::NotFound => tonic::Code::NotFound,
        types::ErrorCode::AlreadyExists => tonic::Code::AlreadyExists,
        types::ErrorCode::InvalidParams => tonic::Code::InvalidArgument,
        types::ErrorCode::BadCredentials => tonic::Code::Unauthenticated,
        types::ErrorCode::Integrity => tonic::Code::DataLoss,
        types::ErrorCode::Conflict => tonic::Code::Aborted,
        types::ErrorCode::Denied => tonic::Code::PermissionDenied,
        types::ErrorCode::Io => tonic::Code::Unavailable,
        types::ErrorCode::Git | types::ErrorCode::Internal => tonic::Code::Internal,
    };
    tonic::Status::with_details(grpc_code, "Latch RPC error", detail.encode_to_vec().into())
}

/// Recover a typed server error, using sanitized static text for transport failures.
pub fn rpc_error(status: tonic::Status) -> types::RpcError {
    if !status.details().is_empty()
        && let Ok(detail) = v1::RpcError::decode(status.details())
    {
        let code = match detail.code {
            1 => types::ErrorCode::Locked,
            2 => types::ErrorCode::NoVault,
            3 => types::ErrorCode::NotFound,
            4 => types::ErrorCode::AlreadyExists,
            5 => types::ErrorCode::InvalidParams,
            6 => types::ErrorCode::BadCredentials,
            7 => types::ErrorCode::Integrity,
            8 => types::ErrorCode::Conflict,
            9 => types::ErrorCode::Denied,
            10 => types::ErrorCode::UnsupportedVersion,
            11 => types::ErrorCode::Git,
            12 => types::ErrorCode::Io,
            _ => types::ErrorCode::Internal,
        };
        return types::RpcError {
            code,
            message: detail.message,
        };
    }
    let (code, message) = match status.code() {
        tonic::Code::PermissionDenied | tonic::Code::Unauthenticated => {
            (types::ErrorCode::Denied, "RPC access denied")
        }
        tonic::Code::Unavailable | tonic::Code::DeadlineExceeded => {
            (types::ErrorCode::Io, "agent unavailable")
        }
        tonic::Code::InvalidArgument => (types::ErrorCode::InvalidParams, "invalid RPC parameters"),
        _ => (types::ErrorCode::Internal, "RPC transport failed"),
    };
    error(code, message)
}
