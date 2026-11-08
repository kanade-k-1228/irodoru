use super::*;
use crate::models::tests::fixture;

fn bundle(root: &Path) -> Result<PathBuf> {
    let source = root.join("source");
    let bundle = root.join("bundle");
    fixture(&source);
    prepare(&source, &bundle)?;
    Ok(bundle)
}
#[test]
fn deterministic_archive_round_trip_and_revision() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = bundle(temp.path())?;
    let destination = temp.path().join("assets");
    let first = distribute(&source, &destination, Some("example/app-models".into()))?;
    assert_eq!(
        first.revision,
        hex(Sha256::digest(fs::read(source.join("bundle.json"))?))
    );
    let archive_path = destination.join(&first.archive.asset);
    let bytes = fs::read(&archive_path)?;
    assert_eq!(first.archive.sha256, hex(Sha256::digest(&bytes)));
    let mut zip = zip::ZipArchive::new(fs::File::open(&archive_path)?)?;
    assert_eq!(zip.len(), first.files.len());
    for entry in &first.files {
        let mut file = zip.by_name(&entry.path)?;
        assert_eq!(file.compression(), CompressionMethod::Stored);
        assert_eq!(file.last_modified(), Some(DateTime::default()));
        assert_eq!(file.unix_mode(), Some(0o100644));
        let mut content = Vec::new();
        file.read_to_end(&mut content)?;
        assert_eq!(content, fs::read(source.join(&entry.path))?);
        fs::File::options()
            .write(true)
            .open(source.join(&entry.path))?
            .set_modified(std::time::UNIX_EPOCH)?;
    }
    drop(zip);
    fs::write(destination.join("stale.part001"), b"stale")?;
    let second = distribute(&source, &destination, None)?;
    assert!(second.repository.is_none());
    assert_eq!(second.archive.sha256, first.archive.sha256);
    assert_eq!(fs::read(&archive_path)?, bytes);
    assert!(!destination.join("stale.part001").exists());
    let (_, mut index) = read_bundle(&source)?;
    index.files.reverse();
    index.checkpoint = "new revision".into();
    json_write(&source.join("bundle.json"), &index)?;
    let changed = distribute(&source, &destination, None)?;
    assert_ne!(changed.revision, first.revision);
    assert_eq!(changed.archive.sha256, first.archive.sha256);
    assert!(!archive_path.exists());
    Ok(())
}
#[test]
fn rejects_corruption_and_layer_limit_without_replacing_good_archive() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = bundle(temp.path())?;
    let destination = temp.path().join("assets");
    let first = distribute(&source, &destination, None)?;
    let (_, index) = read_bundle(&source)?;
    let mut output = tempfile::tempfile()?;
    let total = index.files.iter().map(|e| e.size).sum();
    for limit in [1, total] {
        assert!(
            create_archive(&source, &mut output, &index.files, limit)
                .unwrap_err()
                .to_string()
                .contains("10 GB")
        );
    }
    fs::write(source.join("anime/backbone.onnx.data"), b"changed")?;
    assert!(
        distribute(&source, &destination, None)
            .unwrap_err()
            .to_string()
            .contains("changed while archiving")
    );
    assert_eq!(
        fingerprint(&destination.join(&first.archive.asset))?.1,
        first.archive.sha256
    );
    assert_eq!(fs::read_dir(destination)?.count(), 2);
    Ok(())
}
#[test]
fn rejects_overlapping_directories_and_invalid_repositories() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = bundle(temp.path())?;
    for repository in [
        "../models",
        "OWNER/models",
        "owner/",
        "a/b/c",
        "-a/b",
        "a/b.",
    ] {
        assert!(!valid_repository(repository));
        assert!(
            distribute(
                &source,
                &temp.path().join("assets"),
                Some(repository.into())
            )
            .is_err()
        );
    }
    for destination in [&source, &source.join("nested"), temp.path()] {
        assert!(distribute(&source, destination, None).is_err());
    }
    Ok(())
}
#[test]
fn publication_verifies_local_archive_and_anonymous_manifest_and_blob() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = bundle(temp.path())?;
    let destination = temp.path().join("assets");
    let distribution = distribute(&source, &destination, Some("example/app-models".into()))?;
    let archive = &distribution.archive;
    let layer = serde_json::json!({"digest": format!("sha256:{}", archive.sha256), "size": archive.size,
        "mediaType": "application/zip", "annotations": {"org.opencontainers.image.title": archive.asset}});
    let payload = serde_json::to_vec(&serde_json::json!({"schemaVersion": 2, "layers": [layer]}))?;
    // No network is used. Exercise the same push and anonymous validation contract.
    for failure in ["", "layer", "manifest", "blob", "private"] {
        let mut calls = Vec::new();
        let result = publish(&destination, |args, cwd| {
            assert_eq!(cwd, destination.canonicalize()?);
            calls.push(args[0].clone());
            match args[0].as_str() {
                "push" => {
                    assert_eq!(
                        args[1],
                        format!("ghcr.io/example/app-models:{}", archive.sha256)
                    );
                    let path = option(args, "--export-manifest")?.unwrap();
                    fs::write(
                        path,
                        if failure == "layer" {
                            b"{\"layers\":[]}".as_slice()
                        } else {
                            &payload
                        },
                    )?;
                }
                "manifest" => {
                    assert_eq!(
                        fs::read(option(args, "--registry-config")?.unwrap())?,
                        br#"{"auths":{}}"#
                    );
                    if failure == "private" {
                        bail!("unauthorized");
                    }
                    fs::write(
                        option(args, "--output")?.unwrap(),
                        if failure == "manifest" {
                            b"changed".as_slice()
                        } else {
                            &payload
                        },
                    )?;
                }
                "blob" => {
                    assert!(args.iter().any(|a| a == "--descriptor"));
                    return Ok(if failure == "blob" {
                        b"{}".to_vec()
                    } else {
                        serde_json::to_vec(&layer)?
                    });
                }
                _ => bail!("Unexpected oras command"),
            }
            Ok(Vec::new())
        });
        if failure.is_empty() {
            assert_eq!(result?, format!("sha256:{}", hex(Sha256::digest(&payload))));
            assert_eq!(calls, ["push", "manifest", "blob"]);
        } else {
            assert!(result.is_err(), "{failure}");
        }
    }
    fs::write(destination.join(&archive.asset), b"corrupted")?;
    assert!(
        publish(&destination, |_, _| panic!(
            "Corrupt archive must not be published"
        ))
        .unwrap_err()
        .to_string()
        .contains("checksum mismatch")
    );
    Ok(())
}
