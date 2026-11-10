use crate::domain::InferenceStatus;
use anyhow::{Context, Result, ensure};
use irodoru_model_bundle::{Distribution, Entry};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

const UNPUBLISHED: &str = "このビルドのモデルはまだ公開されていません。モデルが公開済みのリリースを利用するか、開発用モデルを準備してください。";

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
    ensure!(!distribution.is_unpublished(), UNPUBLISHED);
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
