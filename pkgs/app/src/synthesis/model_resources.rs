use crate::domain::InferenceStatus;
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

const REQUIRED: &[&str] = &[
    "anime/LICENSES.txt",
    "anime/provenance.json",
    "codec/LICENSES.txt",
    "codec/provenance.json",
    "anime/manifest.json",
    "anime/tokenizer.json",
    "anime/caption_tokenizer.json",
    "anime/forced_splits.json",
    "anime/backbone.onnx",
    "anime/text_proj.onnx",
    "anime/caption_proj.onnx",
    "anime/speaker_encoder.onnx",
    "anime/duration.onnx",
    "anime/dit_context.onnx",
    "anime/dit_step.onnx",
    "codec/codec.json",
    "codec/dacvae_encoder.onnx",
    "codec/dacvae_decoder.onnx",
];
const UNPUBLISHED: &str = "このビルドのモデルはまだ公開されていません。モデルが公開済みのリリースを利用するか、開発用モデルを準備してください。";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Distribution {
    schema: u32,
    model: String,
    revision: Option<String>,
    repository: Option<String>,
    archive: Option<Archive>,
    files: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    path: String,
    size: u64,
    sha256: String,
}
#[derive(Deserialize)]
struct Archive {
    format: String,
    asset: String,
    size: u64,
    sha256: String,
}

fn checksum(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn filename(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value.starts_with('.')
        && !value.ends_with('.')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}
fn repository(value: &str) -> bool {
    value.len() <= 255
        && value.split('/').count() == 2
        && value.split('/').all(|segment| {
            let alphanumeric = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
            segment.bytes().next().is_some_and(alphanumeric)
                && segment.bytes().last().is_some_and(alphanumeric)
                && segment
                    .bytes()
                    .all(|byte| alphanumeric(byte) || b"._-".contains(&byte))
        })
}
impl Distribution {
    fn validate(&self) -> Result<u64> {
        ensure!(
            self.schema == 2 && self.model == "anime",
            "invalid model distribution"
        );
        if self.revision.is_none()
            && self.files.is_empty()
            && self.repository.is_none()
            && self.archive.is_none()
        {
            bail!(UNPUBLISHED);
        }
        ensure!(
            self.revision.as_deref().is_some_and(checksum),
            "invalid model revision"
        );
        if let Some(name) = &self.repository {
            ensure!(repository(name), "invalid model repository");
        }
        let archive = self.archive.as_ref().context("missing model archive")?;
        ensure!(
            archive.format == "zip"
                && filename(&archive.asset)
                && archive.size > 0
                && archive.size <= 10_000_000_000
                && checksum(&archive.sha256),
            "invalid model archive"
        );
        let mut paths = HashSet::new();
        let mut total = 0u64;
        for entry in &self.files {
            let segments: Vec<_> = entry.path.split('/').collect();
            ensure!(
                segments.len() == 2
                    && matches!(segments[0], "anime" | "codec")
                    && filename(segments[1]),
                "invalid model resource path"
            );
            ensure!(
                paths.insert(entry.path.as_str()),
                "duplicate model resource path"
            );
            ensure!(
                entry.size > 0 && checksum(&entry.sha256),
                "invalid model resource checksum or size"
            );
            total = total
                .checked_add(entry.size)
                .context("model size overflow")?;
        }
        ensure!(total <= archive.size, "model files exceed archive size");
        ensure!(
            REQUIRED.iter().all(|path| paths.contains(path)),
            "incomplete model distribution"
        );
        Ok(total)
    }
}

fn digest(hash: Sha256) -> String {
    hash.finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn verified(path: &Path, size: u64, sha256: &str) -> Result<bool> {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if file.metadata()?.len() != size {
        return Ok(false);
    }
    let mut hash = Sha256::new();
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        let size = file.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        hash.update(&buffer[..size]);
    }
    Ok(digest(hash) == sha256)
}

/// Downloads a single ZIP into an app-owned cache, then installs verified files.
/// Extraction retries reuse the verified archive and completed files; success removes the ZIP.
pub fn download(index: &[u8], root: &Path, progress: &dyn Fn(InferenceStatus)) -> Result<PathBuf> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .redirect_auth_headers(ureq::config::RedirectAuthHeaders::SameHost)
        .timeout_resolve(Some(Duration::from_secs(30)))
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_per_read(Some(Duration::from_secs(60)))
        .build()
        .into();
    let mut token = None;
    install(index, root, progress, |repository, sha256| {
        let response = with_token(
            &mut token,
            || anonymous_token(&agent, repository),
            |token| {
                agent
                    .get(format!(
                        "https://ghcr.io/v2/{repository}/blobs/sha256:{sha256}"
                    ))
                    .header("Authorization", format!("Bearer {token}"))
                    .call()
            },
        )
        .context("モデルをダウンロードできません。接続を確認して再試行してください")?;
        ensure!(
            response.status() == 200,
            "unexpected model download response"
        );
        Ok(Box::new(response.into_parts().1.into_reader()))
    })
}

/// Public GHCR packages use an anonymous, short-lived pull token; no user credentials are needed.
fn anonymous_token(agent: &ureq::Agent, repository: &str) -> Result<String> {
    let mut response = agent
        .get("https://ghcr.io/token")
        .query("service", "ghcr.io")
        .query("scope", format!("repository:{repository}:pull"))
        .call()
        .context("公開モデルにアクセスできません。接続とパッケージの公開状態を確認してください")?;
    ensure!(response.status() == 200, "unexpected model token response");
    let bytes = response
        .body_mut()
        .with_config()
        .limit(64 * 1024)
        .read_to_vec()?;
    parse_token(&bytes)
}

fn parse_token(bytes: &[u8]) -> Result<String> {
    #[derive(Deserialize)]
    struct Token {
        token: Option<String>,
        access_token: Option<String>,
    }
    let response: Token = serde_json::from_slice(bytes)?;
    let token = response
        .token
        .or(response.access_token)
        .context("missing model pull token")?;
    ensure!(
        !token.is_empty() && token.bytes().all(|byte| byte.is_ascii_graphic()),
        "invalid model pull token"
    );
    Ok(token)
}

fn with_token<T>(
    token: &mut Option<String>,
    mut authorize: impl FnMut() -> Result<String>,
    mut request: impl FnMut(&str) -> std::result::Result<T, ureq::Error>,
) -> Result<T> {
    if token.is_none() {
        *token = Some(authorize()?);
    }
    match request(token.as_deref().unwrap()) {
        Err(ureq::Error::StatusCode(401)) => {
            // A large model may take longer than the token lifetime. Refresh once, without looping
            // on missing/private packages or sending a token to a server-provided auth endpoint.
            *token = None;
            *token = Some(authorize()?);
            Ok(request(token.as_deref().unwrap())?)
        }
        result => Ok(result?),
    }
}

fn install(
    index: &[u8],
    root: &Path,
    progress: &dyn Fn(InferenceStatus),
    mut open: impl FnMut(&str, &str) -> Result<Box<dyn Read>>,
) -> Result<PathBuf> {
    let distribution: Distribution = serde_json::from_slice(index)?;
    let total = distribution.validate()?;
    let destination = root.join(distribution.revision.as_ref().unwrap());
    let mut complete = 0u64;
    let mut missing = Vec::new();
    progress(InferenceStatus::Checking { checked: 0, total });
    let mut checked = 0;
    for entry in &distribution.files {
        if verified(&destination.join(&entry.path), entry.size, &entry.sha256)? {
            complete += entry.size;
        } else {
            missing.push(entry);
        }
        checked += entry.size;
        progress(InferenceStatus::Checking { checked, total });
    }
    // A cache remains usable offline, including in unpublished builds with the same revision.
    if missing.is_empty() {
        // Also recover disk space after interruption between the final rename and cleanup.
        let _ = std::fs::remove_file(destination.join(".models.zip"));
        return Ok(destination);
    }
    let archive = distribution.archive.as_ref().unwrap();
    let archive_path = destination.join(".models.zip");
    if !verified(&archive_path, archive.size, &archive.sha256)? {
        let repository = distribution.repository.as_deref().context(UNPUBLISHED)?;
        std::fs::create_dir_all(&destination)?;
        let mut output = tempfile::NamedTempFile::new_in(&destination)?;
        let mut input = open(repository, &archive.sha256)?;
        progress(InferenceStatus::Downloading {
            downloaded: 0,
            total: archive.size,
        });
        copy_verified(
            &mut input,
            &mut output,
            archive.size,
            &archive.sha256,
            |downloaded| {
                progress(InferenceStatus::Downloading {
                    downloaded,
                    total: archive.size,
                });
            },
        )?;
        output.as_file().sync_all()?;
        output.persist(&archive_path).map_err(|error| error.error)?;
    }
    extract_archive(
        &archive_path,
        &destination,
        &distribution.files,
        &missing,
        complete,
        total,
        progress,
    )?;
    let _ = std::fs::remove_file(archive_path);
    Ok(destination)
}

fn copy_verified(
    input: &mut dyn Read,
    output: &mut dyn Write,
    expected_size: u64,
    expected_hash: &str,
    mut progress: impl FnMut(u64),
) -> Result<()> {
    let mut hash = Sha256::new();
    let mut buffer = vec![0; 1024 * 1024];
    let mut bytes = 0u64;
    loop {
        let size = input
            .read(&mut buffer)
            .context("モデルの読み込みが中断されました。再試行してください")?;
        if size == 0 {
            break;
        }
        bytes = bytes
            .checked_add(size as u64)
            .context("model size overflow")?;
        ensure!(bytes <= expected_size, "model exceeds expected size");
        hash.update(&buffer[..size]);
        output
            .write_all(&buffer[..size])
            .context("モデルを保存できません。空き容量を確認してください")?;
        progress(bytes);
    }
    ensure!(
        bytes == expected_size && digest(hash) == expected_hash,
        "モデルの検証に失敗しました。再試行してください"
    );
    Ok(())
}

fn extract_archive(
    archive_path: &Path,
    destination: &Path,
    files: &[Entry],
    missing: &[&Entry],
    mut complete: u64,
    total: u64,
    progress: &dyn Fn(InferenceStatus),
) -> Result<()> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(archive_path)?)?;
    ensure!(
        !archive.has_duplicate_names(),
        "duplicate model archive path"
    );
    ensure!(
        archive.len() == files.len(),
        "unexpected model archive entries"
    );
    let expected: HashMap<_, _> = files
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect();
    let mut indices = HashMap::new();
    // Validate the entire directory before creating files. Paths must exactly match the
    // trusted manifest; links, directories, duplicate names and compressed entries are rejected.
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        let name = entry.name()?.into_owned();
        let expected = expected
            .get(name.as_str())
            .context("unexpected model archive path")?;
        ensure!(
            indices.insert(name, index).is_none(),
            "duplicate model archive path"
        );
        ensure!(
            !entry.is_dir()
                && entry
                    .unix_mode()
                    .is_none_or(|mode| matches!(mode & 0o170000, 0 | 0o100000))
                && entry.compression() == zip::CompressionMethod::Stored
                && entry.size() == expected.size
                && entry.compressed_size() == expected.size,
            "invalid model archive entry"
        );
    }
    progress(InferenceStatus::Extracting {
        extracted: complete,
        total,
    });
    for entry in missing {
        let mut input = archive.by_index(indices[entry.path.as_str()])?;
        let target = destination.join(&entry.path);
        let parent = target.parent().unwrap();
        std::fs::create_dir_all(parent)?;
        let mut output = tempfile::NamedTempFile::new_in(parent)?;
        copy_verified(
            &mut input,
            &mut output,
            entry.size,
            &entry.sha256,
            |bytes| {
                progress(InferenceStatus::Extracting {
                    extracted: complete + bytes,
                    total,
                });
            },
        )?;
        output.as_file().sync_all()?;
        output.persist(&target).map_err(|error| error.error)?;
        complete += entry.size;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use irodoru_engine::output::sha256;
    use std::io::Cursor;

    fn archive() -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .large_file(true);
        for path in REQUIRED {
            zip.start_file(*path, options).unwrap();
            zip.write_all(b"model").unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    fn manifest(archive: &[u8]) -> serde_json::Value {
        serde_json::json!({
            "schema": 2, "model": "anime", "revision": sha256(b"bundle"),
            "repository": "example/models",
            "archive": {"format": "zip", "asset": "models.zip", "size": archive.len(), "sha256": sha256(archive)},
            "files": REQUIRED.iter().map(|path| serde_json::json!({
                "path": path, "size": 5, "sha256": sha256(b"model")
            })).collect::<Vec<_>>()
        })
    }

    #[test]
    fn parses_registry_tokens() {
        for field in ["token", "access_token"] {
            let value = serde_json::json!({field: "pull-token"});
            assert_eq!(
                parse_token(&serde_json::to_vec(&value).unwrap()).unwrap(),
                "pull-token"
            );
        }
        assert!(parse_token(br#"{"token":""}"#).is_err());
    }

    #[test]
    fn expired_token_is_replaced_and_reused() {
        let mut token = Some("expired".into());
        with_token(
            &mut token,
            || Ok("fresh".into()),
            |value| {
                if value == "expired" {
                    Err(ureq::Error::StatusCode(401))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap();
        with_token(
            &mut token,
            || panic!("reuse the token"),
            |value| {
                assert_eq!(value, "fresh");
                Ok(())
            },
        )
        .unwrap();
    }

    #[test]
    fn installs_models_and_reuses_the_cache_offline() {
        let root = tempfile::tempdir().unwrap();
        let archive = archive();
        let index = serde_json::to_vec(&manifest(&archive)).unwrap();
        let destination = install(&index, root.path(), &|_| {}, |_, _| {
            Ok(Box::new(Cursor::new(archive.clone())))
        })
        .unwrap();
        for path in REQUIRED {
            assert_eq!(std::fs::read(destination.join(path)).unwrap(), b"model");
        }
        assert_eq!(
            install(&index, root.path(), &|_| {}, |_, _| {
                panic!("cached models must not download")
            })
            .unwrap(),
            destination
        );
        assert!(!destination.join(".models.zip").exists());
    }

    #[test]
    fn failed_extraction_can_retry_without_downloading() {
        let root = tempfile::tempdir().unwrap();
        let archive = archive();
        let index = serde_json::to_vec(&manifest(&archive)).unwrap();
        let destination = root.path().join(sha256(b"bundle"));
        let blocked = destination.join(REQUIRED[1]);
        assert!(
            install(
                &index,
                root.path(),
                &|status| {
                    if matches!(status, InferenceStatus::Extracting { extracted: 0, .. }) {
                        std::fs::create_dir_all(&blocked).unwrap();
                    }
                },
                |_, _| Ok(Box::new(Cursor::new(archive.clone())))
            )
            .is_err()
        );
        assert!(destination.join(".models.zip").is_file());
        std::fs::remove_dir(&blocked).unwrap();
        install(&index, root.path(), &|_| {}, |_, _| {
            panic!("reuse the verified archive")
        })
        .unwrap();
        assert_eq!(std::fs::read(blocked).unwrap(), b"model");
        assert!(!destination.join(".models.zip").exists());
    }

    #[test]
    fn unsafe_manifest_fails_before_download() {
        let root = tempfile::tempdir().unwrap();
        let mut manifest = manifest(&archive());
        manifest["files"][0]["path"] = "../escape".into();
        assert!(
            install(
                &serde_json::to_vec(&manifest).unwrap(),
                root.path(),
                &|_| {},
                |_, _| { panic!("invalid manifest must not download") }
            )
            .is_err()
        );
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn corrupt_download_is_not_installed() {
        let root = tempfile::tempdir().unwrap();
        let mut archive = archive();
        let index = serde_json::to_vec(&manifest(&archive)).unwrap();
        archive[0] ^= 1;
        assert!(
            install(&index, root.path(), &|_| {}, |_, _| {
                Ok(Box::new(Cursor::new(archive.clone())))
            })
            .is_err()
        );
        let destination = root.path().join(sha256(b"bundle"));
        assert!(!destination.join(REQUIRED[0]).exists());
        assert!(!destination.join(".models.zip").exists());
    }
}
