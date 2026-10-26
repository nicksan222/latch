use prost::Message;
use prost_types::FileDescriptorSet;
use std::{fmt::Write as _, fs, path::PathBuf};

fn snake(name: &str) -> String {
    let mut out = String::new();
    for (index, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && index > 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}
fn pascal(name: &str) -> String {
    name.split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|ch| ch.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    println!("cargo:rustc-env=LATCH_PROTOC_BIN={}", protoc.display());
    // SAFETY: Cargo build scripts run in a dedicated process.
    unsafe { std::env::set_var("PROTOC", protoc) };
    let out = PathBuf::from(std::env::var("OUT_DIR")?);
    let descriptor = out.join("latch_descriptor.bin");
    tonic_prost_build::configure()
        .skip_debug(["."])
        .file_descriptor_set_path(&descriptor)
        .compile_protos(&["proto/latch/v1/agent.proto"], &["proto"])?;
    println!("cargo:rerun-if-changed=proto/latch/v1/agent.proto");
    let set = FileDescriptorSet::decode(fs::read(descriptor)?.as_slice())?;
    let file = set
        .file
        .iter()
        .find(|f| f.package.as_deref() == Some("latch.v1"))
        .ok_or("missing latch.v1")?;
    let service = file
        .service
        .iter()
        .find(|s| s.name.as_deref() == Some("LatchAgent"))
        .ok_or("missing LatchAgent")?;
    let mut debug = String::new();
    for message in &file.message_type {
        let name = message.name.as_deref().ok_or("unnamed message")?;
        writeln!(
            debug,
            "impl std::fmt::Debug for {name} {{ fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{ f.write_str(\"{name}([REDACTED])\") }} }}"
        )?;
        for (index, oneof) in message.oneof_decl.iter().enumerate() {
            if message.field.iter().any(|field| {
                field.oneof_index == Some(index as i32) && !field.proto3_optional.unwrap_or(false)
            }) {
                let module = snake(name);
                let variant = pascal(oneof.name.as_deref().ok_or("unnamed oneof")?);
                writeln!(
                    debug,
                    "impl std::fmt::Debug for {module}::{variant} {{ fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{ f.write_str(\"{name}::{variant}([REDACTED])\") }} }}"
                )?;
            }
        }
    }
    for enumeration in &file.enum_type {
        let name = enumeration.name.as_deref().ok_or("unnamed enum")?;
        writeln!(
            debug,
            "impl std::fmt::Debug for {name} {{ fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{ f.write_str(self.as_str_name()) }} }}"
        )?;
    }
    fs::write(out.join("latch_debug.rs"), debug)?;
    let methods = service
        .method
        .iter()
        .filter(|m| !m.server_streaming.unwrap_or(false))
        .map(|method| {
            Ok((
                method.name.as_deref().ok_or("unnamed method")?,
                method
                    .input_type
                    .as_deref()
                    .and_then(|s| s.strip_prefix(".latch.v1."))
                    .ok_or("invalid input")?,
                method
                    .output_type
                    .as_deref()
                    .and_then(|s| s.strip_prefix(".latch.v1."))
                    .ok_or("invalid output")?,
            ))
        })
        .collect::<Result<Vec<_>, &str>>()?;
    let mut generated = String::from(
        "use crate::{errors::RpcError, v1};\nuse prost::Message;\n\n#[derive(Clone, Debug)]\n#[allow(clippy::large_enum_variant)]\npub enum Call {\n",
    );
    for (name, input, _) in &methods {
        writeln!(generated, "    {name}(v1::{input}),")?;
    }
    generated.push_str(
        "}\n#[derive(Clone, Debug)]\n#[allow(clippy::large_enum_variant)]\npub enum Reply {\n",
    );
    for (name, _, output) in &methods {
        writeln!(generated, "    {name}(v1::{output}),")?;
    }
    generated.push_str("}\nimpl Call {\n pub fn method(&self) -> &'static str { match self {\n");
    for (name, _, _) in &methods {
        writeln!(generated, "Self::{name}(..) => {name:?},")?;
    }
    generated.push_str("} }\n pub fn decode(method: &str, bytes: &[u8]) -> Result<Self, RpcError> { match method {\n");
    for (name, input, _) in &methods {
        writeln!(
            generated,
            "{name:?} => v1::{input}::decode(bytes).map(Self::{name}).map_err(|_| RpcError::invalid(\"invalid protobuf request\")),"
        )?;
    }
    generated.push_str("_ => Err(RpcError::invalid(\"unknown RPC method\")),\n} }\n pub fn encode(&self) -> Vec<u8> { match self {\n");
    for (name, _, _) in &methods {
        writeln!(
            generated,
            "Self::{name}(message) => message.encode_to_vec(),"
        )?;
    }
    generated.push_str("} }\n}\nimpl Reply {\n pub fn encode(&self) -> Vec<u8> { match self {\n");
    for (name, _, _) in &methods {
        writeln!(
            generated,
            "Self::{name}(message) => message.encode_to_vec(),"
        )?;
    }
    generated.push_str("} }\n}\npub trait RpcSdk: RpcTransport {\n");
    for (name, input, output) in &methods {
        let method = snake(name);
        let (arg, value) = if *input == "Empty" {
            (String::new(), "v1::Empty {}".to_owned())
        } else {
            (format!(", params: v1::{input}"), "params".to_owned())
        };
        writeln!(
            generated,
            "fn {method}(&mut self{arg}) -> Result<v1::{output}, Self::Error> {{ match self.execute(Call::{name}({value}))? {{ Reply::{name}(value) => Ok(value), _ => Err(RpcError::internal(\"unexpected RPC response\").into()), }} }}"
        )?;
    }
    generated.push_str("}\nimpl<T: RpcTransport> RpcSdk for T {}\n");
    fs::write(out.join("latch_sdk.rs"), generated)?;
    let mut client = String::from(
        "pub async fn execute(client: &mut crate::v1::latch_agent_client::LatchAgentClient<tonic::transport::Channel>, call: crate::sdk::Call, identity: &crate::v1::HelloParams) -> Result<crate::sdk::Reply, crate::errors::RpcError> {\nuse crate::sdk::{Call, Reply};\nmatch call {\n",
    );
    let mut macro_source = String::from(
        "#[macro_export]\nmacro_rules! for_each_rpc { ($callback:ident) => { $callback! {\n",
    );
    for (name, input, output) in &methods {
        let method = snake(name);
        writeln!(
            client,
            "Call::{name}(params) => client.{method}(request(params, identity)?).await.map(|response| Reply::{name}(response.into_inner())).map_err(crate::errors::rpc_error),"
        )?;
        writeln!(macro_source, "({method}, {name}, {input}, {output});")?;
    }
    client.push_str("}\n}\n");
    macro_source.push_str("} }; }\n");
    fs::write(out.join("latch_client.rs"), client)?;
    fs::write(out.join("latch_methods.rs"), macro_source)?;
    Ok(())
}
