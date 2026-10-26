//! Json import entrypoint; parsing support is shared internally.

use std::path::Path;

use crate::core::Result;
use crate::core::import::{Parsed, generic};

fn parse(path: &Path, _passphrase: Option<&str>) -> Result<Parsed> {
    generic::parse(path, generic::Kind::Json)
}

import_kind! { format: Json, id: "json", parse: parse }
