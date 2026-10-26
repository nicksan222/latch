// Export boundary regression; discovered by the import_export_feature target.
#[test]
fn failed_export_persist_preserves_directory_and_removes_temporary_file() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("destination");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("keep.txt"), b"existing contents").unwrap();

    let err = latch::core::export::write_private(&destination, b"new export").unwrap_err();
    assert!(!err.to_string().contains("do-not-leak-this-secret-value"));
    assert_eq!(
        std::fs::read(destination.join("keep.txt")).unwrap(),
        b"existing contents"
    );
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(".latch-export-"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "temporary exports remain: {leftovers:?}"
    );
}
