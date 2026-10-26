//! BrowserCsv import entrypoint; parsing support is shared internally.

use std::path::Path;

use crate::core::Result;
use crate::core::import::{Parsed, csv_formats};

fn parse(path: &Path, _passphrase: Option<&str>) -> Result<Parsed> {
    csv_formats::parse_browser(path)
}

import_kind! { format: BrowserCsv, id: "browser_csv", parse: parse }
