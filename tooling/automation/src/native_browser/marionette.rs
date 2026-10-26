use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::io::{Read, Write};

const MAX_PACKET: usize = 16 * 1024 * 1024;

pub(crate) fn write_packet<W: Write>(writer: &mut W, value: &Value) -> Result<()> {
    let body = serde_json::to_vec(value).context("encode Marionette packet")?;
    write!(writer, "{}:", body.len()).context("write Marionette packet length")?;
    writer.write_all(&body).context("write Marionette packet")?;
    writer.flush().context("flush Marionette packet")?;
    Ok(())
}

pub(crate) fn read_packet<R: Read>(reader: &mut R) -> Result<Value> {
    let mut length = Vec::with_capacity(8);
    loop {
        let mut byte = [0_u8; 1];
        reader
            .read_exact(&mut byte)
            .context("read Marionette packet length")?;
        match byte[0] {
            b':' => break,
            b'0'..=b'9' if length.len() < 10 => length.push(byte[0]),
            _ => bail!("invalid Marionette packet length"),
        }
    }
    if length.is_empty() {
        bail!("empty Marionette packet length");
    }
    let length: usize = std::str::from_utf8(&length)
        .context("decode Marionette packet length")?
        .parse()
        .context("parse Marionette packet length")?;
    if length > MAX_PACKET {
        bail!("Marionette packet exceeds the size limit");
    }
    let mut body = vec![0; length];
    reader
        .read_exact(&mut body)
        .context("read Marionette packet body")?;
    serde_json::from_slice(&body).context("decode Marionette packet")
}

pub(crate) fn response(value: Value, expected_id: u32) -> Result<Value> {
    let packet = value
        .as_array()
        .context("Marionette response is not an array")?;
    if packet.len() != 4 || packet[0].as_u64() != Some(1) {
        bail!("invalid Marionette response packet");
    }
    if packet[1].as_u64() != Some(u64::from(expected_id)) {
        bail!("Marionette response message id mismatch");
    }
    if !packet[2].is_null() {
        let error = packet[2]
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("remote error");
        bail!("Marionette command failed: {error}");
    }
    Ok(packet[3].clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn length_prefixed_packet_round_trips() {
        let packet = json!([0, 3, "WebDriver:Navigate", {"url":"http://127.0.0.1/"}]);
        let mut encoded = Vec::new();
        write_packet(&mut encoded, &packet).unwrap();

        let decoded = read_packet(&mut encoded.as_slice()).unwrap();

        assert_eq!(decoded, packet);
    }

    #[test]
    fn response_requires_matching_id_and_no_remote_error() {
        assert_eq!(
            response(json!([1, 7, null, {"value": "ok"}]), 7).unwrap(),
            json!({"value":"ok"})
        );
        assert!(response(json!([1, 8, null, null]), 7).is_err());
        assert!(response(json!([1, 7, {"error":"invalid argument"}, null]), 7).is_err());
    }
}
