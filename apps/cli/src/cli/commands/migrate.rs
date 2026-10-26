use super::*;

pub(super) fn migrate() -> CliResult {
    outln!("vault format v1 is current; nothing to migrate");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_format_migration_does_not_require_a_session() {
        migrate().unwrap();
    }
}
