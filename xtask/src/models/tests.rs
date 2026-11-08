use super::*;

pub(super) fn fixture(directory: &Path) {
    for (folder, names) in REQUIRED {
        fs::create_dir_all(directory.join(folder)).unwrap();
        for name in *names {
            fs::write(directory.join(folder).join(name), b"model fixture").unwrap();
        }
    }
    fs::write(
        directory.join("anime/manifest.json"),
        br#"{"name":"anime","checkpoint":"fixture"}"#,
    )
    .unwrap();
    fs::write(directory.join("anime/backbone.onnx.data"), [42; 1024]).unwrap();
}
#[test]
fn prepare_preserves_unchanged_files_and_repairs_corruption() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    let bundle = temp.path().join("bundle");
    fixture(&source);
    fs::write(source.join("anime/backbone.portable.onnx"), b"unused")?;
    prepare(&source, &bundle)?;
    check(&bundle)?;
    assert!(!bundle.join("anime/backbone.portable.onnx").exists());
    let weights = bundle.join("anime/backbone.onnx.data");
    assert_eq!(fs::read(&weights)?, vec![42; 1024]);
    let modified = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
    fs::File::options()
        .write(true)
        .open(&weights)?
        .set_modified(modified)?;
    prepare(&source, &bundle)?;
    assert_eq!(fs::metadata(&weights)?.modified()?, modified);
    fs::write(&weights, b"corrupt")?;
    assert!(
        check(&bundle)
            .unwrap_err()
            .to_string()
            .contains("checksum mismatch")
    );
    prepare(&source, &bundle)?;
    fs::write(bundle.join("stale"), b"stale")?;
    fs::write(
        bundle.join("anime/backbone.onnx.data.copy-tmp"),
        b"interrupted",
    )?;
    assert!(check(&bundle).is_err());
    prepare(&source, &bundle)?;
    check(&bundle)?;
    assert!(!bundle.join("stale").exists());
    Ok(())
}
#[test]
fn prepare_rejects_overlap_missing_and_empty_models() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    let destination = temp.path().join("destination");
    fixture(&source);
    for path in [&source, &source.join("nested"), temp.path()] {
        assert!(
            prepare(&source, path)
                .unwrap_err()
                .to_string()
                .contains("overlap")
        );
    }
    let required = source.join("anime/backbone.onnx");
    fs::remove_file(&required)?;
    assert!(
        prepare(&source, &destination)
            .unwrap_err()
            .to_string()
            .contains("Incomplete")
    );
    fs::write(required, b"")?;
    assert!(
        prepare(&source, &destination)
            .unwrap_err()
            .to_string()
            .contains("Empty")
    );
    assert!(!destination.exists());
    Ok(())
}
#[test]
fn rejects_invalid_bundle_metadata() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    let bundle = temp.path().join("bundle");
    fixture(&source);
    prepare(&source, &bundle)?;
    let original = fs::read(bundle.join("bundle.json"))?;
    for path in [
        "../secret",
        "anime/../../x",
        "anime/..",
        "anime/C:x",
        "anime/\\x",
        "anime/file.",
        "anime/.hidden",
        "anime/a/b",
    ] {
        let mut index: Index = serde_json::from_slice(&original)?;
        index.files[0].path = path.into();
        json_write(&bundle.join("bundle.json"), &index)?;
        assert!(check(&bundle).is_err(), "{path}");
        assert!(distribution::distribute(&bundle, &temp.path().join("assets"), None).is_err());
    }
    for change in ["duplicate", "digest", "size", "missing"] {
        let mut index: Index = serde_json::from_slice(&original)?;
        match change {
            "duplicate" => index.files[0].path = index.files[1].path.clone(),
            "digest" => index.files[0].sha256 = "A".repeat(64),
            "size" => index.files[0].size = MAX_ARCHIVE_SIZE + 1,
            _ => {
                index.files.pop();
            }
        }
        assert!(validate_files(&index.files).is_err(), "{change}");
    }
    Ok(())
}
#[cfg(unix)]
#[test]
fn rejects_symlinks_before_copying() -> Result<()> {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    let bundle = temp.path().join("bundle");
    let outside = temp.path().join("outside");
    fixture(&source);
    prepare(&source, &bundle)?;
    fs::write(&outside, b"do not overwrite")?;
    let target = bundle.join("anime/backbone.onnx");
    fs::remove_file(&target)?;
    symlink(&outside, &target)?;
    assert!(prepare(&source, &bundle).is_err());
    assert_eq!(fs::read(&outside)?, b"do not overwrite");
    fs::remove_file(target)?;
    symlink(&outside, source.join("anime/link"))?;
    assert!(prepare(&source, &bundle).is_err());
    Ok(())
}
