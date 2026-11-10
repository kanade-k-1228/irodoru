use super::{copy_hashed, fingerprint, hex, read_bundle, separate_directories};
use crate::json_write;
use anyhow::{Context, Result};
use irodoru_model_bundle::{Archive, Distribution, Entry, MAX_ARCHIVE_SIZE, valid_repository};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
use zip::{CompressionMethod, DateTime, ZipWriter, write::SimpleFileOptions};

fn create_archive(
    source: &Path,
    output: &mut fs::File,
    entries: &[Entry],
    limit: u64,
) -> Result<()> {
    let mut entries: Vec<_> = entries.iter().collect();
    entries.sort_by_key(|entry| &entry.path);
    anyhow::ensure!(
        entries.iter().map(|e| e.size).sum::<u64>() <= limit,
        "Model archive exceeds GHCR's 10 GB layer limit"
    );
    let mut archive = ZipWriter::new(output);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644)
        .large_file(true);
    for entry in entries {
        archive.start_file(&entry.path, options)?;
        anyhow::ensure!(
            copy_hashed(&source.join(&entry.path), &mut archive)?
                == (entry.size, entry.sha256.clone()),
            "Model changed while archiving: {}",
            entry.path
        );
    }
    anyhow::ensure!(
        archive.finish()?.metadata()?.len() <= limit,
        "Model archive exceeds GHCR's 10 GB layer limit"
    );
    Ok(())
}
pub(super) fn distribute(
    source: &Path,
    destination: &Path,
    repository: Option<String>,
) -> Result<Distribution> {
    anyhow::ensure!(
        repository.as_deref().is_none_or(valid_repository),
        "Invalid GHCR model repository; expected lowercase owner/package"
    );
    let (bytes, index) = read_bundle(source)?;
    let (source, destination) = separate_directories(source, destination)?;
    let revision = hex(Sha256::digest(bytes));
    let asset = format!("{revision}-models.zip");
    let mut temporary = tempfile::NamedTempFile::new_in(&destination)?;
    create_archive(
        &source,
        temporary.as_file_mut(),
        &index.files,
        MAX_ARCHIVE_SIZE,
    )?;
    let (size, sha256) = fingerprint(temporary.path())?;
    temporary.persist(destination.join(&asset))?;
    let manifest = Distribution {
        schema: 2,
        model: "anime".into(),
        revision: Some(revision),
        repository,
        archive: Some(Archive {
            format: "zip".into(),
            asset: asset.clone(),
            size,
            sha256,
        }),
        files: index.files,
    };
    manifest.validate()?;
    let temporary = tempfile::NamedTempFile::new_in(&destination)?;
    json_write(temporary.path(), &manifest)?;
    temporary.persist(destination.join("model-distribution.json"))?;
    for entry in fs::read_dir(&destination)? {
        let path = entry?.path();
        if path.is_file()
            && path
                .file_name()
                .is_some_and(|n| n != asset.as_str() && n != "model-distribution.json")
        {
            fs::remove_file(path)?;
        }
    }
    Ok(manifest)
}
pub(super) fn publish(
    directory: &Path,
    mut oras: impl FnMut(&[String], &Path) -> Result<Vec<u8>>,
) -> Result<String> {
    let directory = directory.canonicalize()?;
    let distribution: Distribution =
        serde_json::from_slice(&fs::read(directory.join("model-distribution.json"))?)?;
    let repository = distribution
        .repository
        .as_deref()
        .context("Missing GHCR repository")?;
    distribution.validate()?;
    let revision = distribution.revision.as_deref().unwrap();
    let archive = distribution.archive.as_ref().unwrap();
    anyhow::ensure!(
        archive.asset == format!("{revision}-models.zip"),
        "Invalid model archive"
    );
    anyhow::ensure!(
        fingerprint(&directory.join(&archive.asset))? == (archive.size, archive.sha256.clone()),
        "Model archive checksum mismatch"
    );
    let temporary = tempfile::tempdir()?;
    let exported = temporary.path().join("manifest.json");
    let reference = format!("ghcr.io/{repository}");
    let blob_digest = format!("sha256:{}", archive.sha256);
    oras(
        &[
            "push".into(),
            format!("{reference}:{}", archive.sha256),
            "--artifact-type".into(),
            "application/vnd.irodoru.models.v2".into(),
            "--annotation".into(),
            "org.opencontainers.image.created=1970-01-01T00:00:00Z".into(),
            "--annotation".into(),
            format!(
                "org.opencontainers.image.source=https://github.com/{}",
                repository.strip_suffix("-models").unwrap_or(repository)
            ),
            "--annotation".into(),
            format!("org.opencontainers.image.revision={revision}"),
            "--export-manifest".into(),
            exported.to_string_lossy().into_owned(),
            format!("{}:application/zip", archive.asset),
        ],
        &directory,
    )?;
    let manifest_bytes = fs::read(exported)?;
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes)?;
    let layers = manifest["layers"]
        .as_array()
        .context("Missing OCI layers")?;
    anyhow::ensure!(
        layers.len() == 1
            && layers[0]["digest"] == blob_digest
            && layers[0]["size"] == archive.size
            && layers[0]["mediaType"] == "application/zip"
            && layers[0]["annotations"]["org.opencontainers.image.title"] == archive.asset,
        "Published OCI layer does not match the app model archive"
    );
    let digest = format!("sha256:{}", hex(Sha256::digest(&manifest_bytes)));
    let anonymous = temporary.path().join("anonymous.json");
    fs::write(&anonymous, br#"{"auths":{}}"#)?;
    let fetched = temporary.path().join("public-manifest.json");
    let public_access = format!(
        "Cannot download {reference} anonymously. Set the package visibility to Public in GitHub Package settings, then rerun this workflow."
    );
    oras(
        &[
            "manifest".into(),
            "fetch".into(),
            format!("{reference}@{digest}"),
            "--registry-config".into(),
            anonymous.to_string_lossy().into_owned(),
            "--output".into(),
            fetched.to_string_lossy().into_owned(),
        ],
        &directory,
    )
    .context(public_access.clone())?;
    // HEAD checks public access without downloading several GB again.
    let descriptor: serde_json::Value = serde_json::from_slice(
        &oras(
            &[
                "blob".into(),
                "fetch".into(),
                format!("{reference}@{blob_digest}"),
                "--descriptor".into(),
                "--registry-config".into(),
                anonymous.to_string_lossy().into_owned(),
            ],
            &directory,
        )
        .context(public_access)?,
    )?;
    anyhow::ensure!(
        fs::read(fetched)? == manifest_bytes,
        "Public OCI manifest digest mismatch"
    );
    anyhow::ensure!(
        descriptor["digest"] == blob_digest && descriptor["size"] == archive.size,
        "Public OCI blob descriptor mismatch"
    );
    println!("Public model artifact verified: {reference}@{digest}");
    Ok(digest)
}
