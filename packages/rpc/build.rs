use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use prost::Message;
use prost_types::FileDescriptorSet;

#[path = "build/compat.rs"]
mod compat;

fn annotation(comments: &str, key: &str) -> Result<String, Box<dyn std::error::Error>> {
    comments
        .split(';')
        .find_map(|part| part.trim().strip_prefix(key).map(str::trim))
        .map(str::to_owned)
        .ok_or_else(|| format!("RPC method missing {key} annotation").into())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    println!("cargo:rustc-env=LATCH_PROTOC_BIN={}", protoc.display());
    // SAFETY: build scripts run in an isolated process before compilation.
    unsafe { std::env::set_var("PROTOC", protoc) };
    let out = PathBuf::from(std::env::var("OUT_DIR")?);
    let descriptor = out.join("latch_descriptor.bin");
    tonic_prost_build::configure()
        .skip_debug(["."])
        .file_descriptor_set_path(&descriptor)
        .compile_protos(&["proto/latch/v1/agent.proto"], &["proto"])?;
    println!("cargo:rerun-if-changed=proto/latch/v1/agent.proto");

    let set = FileDescriptorSet::decode(fs::read(&descriptor)?.as_slice())?;
    let file = set
        .file
        .iter()
        .find(|f| f.package.as_deref() == Some("latch.v1"))
        .ok_or("missing latch.v1 descriptor")?;
    let service = file
        .service
        .iter()
        .find(|s| s.name.as_deref() == Some("LatchAgent"))
        .ok_or("missing LatchAgent service")?;
    fs::write(out.join("latch_compat_types.rs"), compat::generate(file))?;
    let locations = file
        .source_code_info
        .as_ref()
        .ok_or("missing protobuf source info")?;
    let mut debug_source = String::new();
    for message in &file.message_type {
        let name = message.name.as_deref().ok_or("unnamed protobuf message")?;
        writeln!(
            debug_source,
            "impl std::fmt::Debug for {name} {{ fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{ f.write_str(\"{name}([REDACTED])\") }} }}"
        )?;
        for (index, oneof) in message.oneof_decl.iter().enumerate() {
            if message.field.iter().any(|field| {
                field.oneof_index == Some(index as i32) && !field.proto3_optional.unwrap_or(false)
            }) {
                let oneof_name = oneof.name.as_deref().ok_or("unnamed protobuf oneof")?;
                let type_name = oneof_name
                    .chars()
                    .enumerate()
                    .map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c })
                    .collect::<String>();
                let module_name = name
                    .chars()
                    .enumerate()
                    .flat_map(|(i, c)| {
                        if c.is_uppercase() && i > 0 {
                            vec!['_', c.to_ascii_lowercase()]
                        } else {
                            vec![c.to_ascii_lowercase()]
                        }
                    })
                    .collect::<String>();
                writeln!(
                    debug_source,
                    "impl std::fmt::Debug for {module_name}::{type_name} {{ fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{ f.write_str(\"{name}::{type_name}([REDACTED])\") }} }}"
                )?;
            }
        }
    }
    let mut narrow_source = String::from(
        "fn narrow_bound(message: &str, field: &str) -> Option<u64> { match (message, field) {\n",
    );
    let mut omit_source = String::from(
        "fn legacy_omittable(message: &str, field: &str) -> bool { matches!((message, field),\n",
    );
    for (message_index, message) in file.message_type.iter().enumerate() {
        for (field_index, field) in message.field.iter().enumerate() {
            let Some(location) = locations
                .location
                .iter()
                .find(|location| location.path == [4, message_index as i32, 2, field_index as i32])
            else {
                continue;
            };
            let comments = format!(
                "{} {}",
                location.leading_comments.as_deref().unwrap_or(""),
                location.trailing_comments.as_deref().unwrap_or("")
            );
            let message_name = message.name.as_deref().ok_or("unnamed message")?;
            let field_name = field.json_name.as_deref().ok_or("unnamed field")?;
            if comments.contains("legacy_omittable: true") {
                writeln!(
                    omit_source,
                    "    ({:?}, {:?}) |",
                    format!("latch.v1.{message_name}"),
                    field_name
                )?;
            }
            let Some(bits) = comments
                .split("legacy_uint_bits:")
                .nth(1)
                .and_then(|text| text.split_whitespace().next())
                .and_then(|text| text.parse::<u32>().ok())
            else {
                continue;
            };
            let max = (1_u64 << bits) - 1;
            writeln!(
                narrow_source,
                "    ({:?}, {:?}) => Some({max}),",
                format!("latch.v1.{message_name}"),
                field_name
            )?;
        }
    }
    narrow_source.push_str("    _ => None,\n} }\n");
    omit_source.push_str("    (\"\", \"\")) }\n");
    let mut macro_source = String::from(
        "#[macro_export]\nmacro_rules! for_each_rpc {\n    ($apply:ident) => { $apply! {\n",
    );
    let mut sdk_source = String::from("pub mod methods {\n");
    let mut sdk_trait = String::from("pub trait RpcSdk: RpcTransport {\n");
    let mut all_aliases = Vec::new();
    let mut client_source = String::from(
        "use serde_json::Value;\nuse tonic::transport::Channel;\nuse crate::{codec, types, v1};\n\npub fn request<T>(message: T, hello: &types::HelloParams) -> Result<tonic::Request<T>, types::RpcError> {\n    let mut request = tonic::Request::new(message);\n    let client = serde_json::to_value(hello.client).ok().and_then(|v| v.as_str().map(str::to_owned)).ok_or_else(|| types::RpcError { code: types::ErrorCode::InvalidParams, message: \"invalid client kind\".into() })?;\n    request.metadata_mut().insert(\"x-latch-client\", client.parse().map_err(|_| types::RpcError { code: types::ErrorCode::InvalidParams, message: \"invalid client kind\".into() })?);\n    request.metadata_mut().insert(\"x-latch-version\", hello.version.parse().map_err(|_| types::RpcError { code: types::ErrorCode::InvalidParams, message: \"invalid client version\".into() })?);\n    Ok(request)\n}\n\npub async fn call(client: &mut v1::latch_agent_client::LatchAgentClient<Channel>, method: &str, params: Value, hello: &types::HelloParams) -> Result<Value, types::RpcError> {\n    match method {\n",
    );
    for (index, method) in service.method.iter().enumerate() {
        if method.server_streaming.unwrap_or(false) {
            continue;
        }
        let location = locations
            .location
            .iter()
            .find(|l| l.path == [6, 0, 2, index as i32])
            .ok_or("missing method source location")?;
        let comments = location
            .leading_comments
            .as_deref()
            .ok_or("missing method annotations")?;
        let alias = annotation(comments, "legacy:")?;
        let rust_method = annotation(comments, "rust:")?;
        let params_mode = annotation(comments, "params:")?;
        let request_type = method
            .input_type
            .as_deref()
            .ok_or("missing RPC input type")?
            .strip_prefix(".latch.v1.")
            .ok_or("RPC input outside latch.v1")?;
        let response_type = method
            .output_type
            .as_deref()
            .ok_or("missing RPC output type")?
            .strip_prefix(".latch.v1.")
            .ok_or("RPC output outside latch.v1")?;
        all_aliases.push(alias.clone());
        writeln!(
            sdk_source,
            "    pub const {}: &str = {alias:?};",
            rust_method.to_ascii_uppercase()
        )?;
        let result = if response_type == "Empty" {
            "()".to_owned()
        } else if let Some(wrapper) = file
            .message_type
            .iter()
            .find(|m| m.name.as_deref() == Some(response_type))
            .filter(|m| m.field.len() == 1 && m.field[0].name.as_deref() == Some("items"))
        {
            let item = &wrapper.field[0];
            let ty = item
                .type_name
                .as_deref()
                .and_then(|n| n.strip_prefix(".latch.v1."))
                .map(|name| format!("crate::types::{name}"))
                .unwrap_or_else(|| "String".to_owned());
            format!("Vec<{ty}>")
        } else {
            format!("crate::types::{response_type}")
        };
        if request_type == "Empty" {
            let params = if params_mode == "empty_object" {
                "serde_json::json!({})"
            } else {
                "()"
            };
            writeln!(
                sdk_trait,
                "    fn {rust_method}(&mut self) -> Result<{result}, Self::Error> {{ self.request({alias:?}, {params}) }}"
            )?;
        } else {
            writeln!(
                sdk_trait,
                "    fn {rust_method}(&mut self, params: crate::types::{request_type}) -> Result<{result}, Self::Error> {{ self.request({alias:?}, params) }}"
            )?;
        }
        writeln!(
            macro_source,
            "        ({rust_method}, {alias:?}, {request_type}, {response_type});"
        )?;
        writeln!(
            client_source,
            "        {alias:?} => {{ let message: v1::{request_type} = codec::from_legacy(params, \"latch.v1.{request_type}\")?; let response = client.{rust_method}(request(message, hello)?).await.map_err(codec::rpc_error)?.into_inner(); codec::to_legacy(&response, \"latch.v1.{response_type}\") }}"
        )?;
    }
    macro_source.push_str("    } };\n}\n");
    client_source.push_str("        _ => Err(types::RpcError { code: types::ErrorCode::InvalidParams, message: \"unknown RPC procedure\".into() }),\n    }\n}\n");
    write!(sdk_source, "    pub const ALL: &[&str] = &[")?;
    for alias in &all_aliases {
        write!(sdk_source, "{alias:?},")?;
    }
    sdk_source.push_str("];\n}\n");
    sdk_trait.push_str("}\nimpl<T: RpcTransport> RpcSdk for T {}\n");
    sdk_source.push_str(&sdk_trait);
    fs::write(out.join("latch_methods.rs"), macro_source)?;
    fs::write(out.join("latch_client.rs"), client_source)?;
    fs::write(out.join("latch_sdk.rs"), sdk_source)?;
    fs::write(out.join("latch_narrow.rs"), narrow_source)?;
    fs::write(out.join("latch_omit.rs"), omit_source)?;
    fs::write(out.join("latch_debug.rs"), debug_source)?;
    Ok(())
}
