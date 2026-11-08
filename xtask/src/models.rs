use super::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    io::{Read, Write},
};

mod distribution;
#[cfg(test)]
mod tests;

const MAX_ARCHIVE_SIZE: u64 = 10_000_000_000;
const REQUIRED: &[(&str, &[&str])] = &[
    (
        "anime",
        &[
            "manifest.json",
            "tokenizer.json",
            "caption_tokenizer.json",
            "forced_splits.json",
            "backbone.onnx",
            "text_proj.onnx",
            "caption_proj.onnx",
            "speaker_encoder.onnx",
            "duration.onnx",
            "dit_context.onnx",
            "dit_step.onnx",
        ],
    ),
    (
        "codec",
        &["codec.json", "dacvae_encoder.onnx", "dacvae_decoder.onnx"],
    ),
];

#[derive(Debug, Serialize, Deserialize)]
struct Entry {
    path: String,
    size: u64,
    sha256: String,
}
#[derive(Debug, Serialize, Deserialize)]
struct Index {
    model: String,
    checkpoint: serde_json::Value,
    files: Vec<Entry>,
}
fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
// Hash while copying so archiving does not need a second pass over the source.
fn copy_hashed(path: &Path, output: &mut impl Write) -> Result<(u64, String)> {
    let mut input = fs::File::open(path).with_context(|| format!("Read {}", path.display()))?;
    let mut hash = Sha256::new();
    let mut size = 0;
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        output.write_all(&buffer[..n])?;
        hash.update(&buffer[..n]);
        size += n as u64;
    }
    Ok((size, hex(hash.finalize())))
}
fn fingerprint(path: &Path) -> Result<(u64, String)> {
    copy_hashed(path, &mut std::io::sink())
}
fn files(directory: &Path) -> Result<BTreeSet<PathBuf>> {
    let mut result = BTreeSet::new();
    anyhow::ensure!(!directory.is_symlink(), "Symlink in model resources");
    if !directory.exists() {
        return Ok(result);
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            result.extend(files(&entry.path())?);
        } else if kind.is_file() {
            result.insert(entry.path());
        } else {
            bail!("Unsupported model resource: {}", entry.path().display());
        }
    }
    Ok(result)
}
fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn valid_path(path: &str) -> bool {
    let Some((folder, name)) = path.split_once('/') else {
        return false;
    };
    let edge = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'-';
    ["anime", "codec"].contains(&folder)
        && name.bytes().next().is_some_and(edge)
        && name.bytes().last().is_some_and(edge)
        && name.bytes().all(|b| edge(b) || b == b'.')
}
fn validate_files(entries: &[Entry]) -> Result<BTreeSet<&str>> {
    let mut paths = BTreeSet::new();
    for entry in entries {
        anyhow::ensure!(valid_path(&entry.path), "Invalid distribution model path");
        anyhow::ensure!(
            paths.insert(entry.path.as_str()),
            "Duplicate model resource"
        );
        anyhow::ensure!(
            entry.size > 0 && entry.size <= MAX_ARCHIVE_SIZE,
            "Empty or invalid model file"
        );
        anyhow::ensure!(valid_sha256(&entry.sha256), "Invalid model file checksum");
    }
    for (folder, names) in REQUIRED {
        for name in *names {
            anyhow::ensure!(
                paths.contains(format!("{folder}/{name}").as_str()),
                "Incomplete model resources: {folder}/{name}"
            );
        }
    }
    Ok(paths)
}
fn separate_directories(source: &Path, destination: &Path) -> Result<(PathBuf, PathBuf)> {
    let source = source.canonicalize()?;
    fs::create_dir_all(destination)?;
    let destination = destination.canonicalize()?;
    anyhow::ensure!(
        !source.starts_with(&destination) && !destination.starts_with(&source),
        "Model directories must not overlap"
    );
    Ok((source, destination))
}
fn read_bundle(source: &Path) -> Result<(Vec<u8>, Index)> {
    let bytes = fs::read(source.join("bundle.json"))?;
    let index: Index = serde_json::from_slice(&bytes)?;
    anyhow::ensure!(index.model == "anime", "App model resources must use anime");
    let expected = validate_files(&index.files)?
        .into_iter()
        .chain(["bundle.json"])
        .map(|path| source.join(path))
        .collect();
    anyhow::ensure!(
        files(source)? == expected,
        "Unexpected model resources; run cargo xtask models prepare"
    );
    Ok((bytes, index))
}
fn check(source: &Path) -> Result<()> {
    let (_, index) = read_bundle(source)?;
    for entry in index.files {
        anyhow::ensure!(
            fingerprint(&source.join(&entry.path))? == (entry.size, entry.sha256),
            "Model checksum mismatch: {}",
            entry.path
        );
    }
    println!("Anime model resources verified");
    Ok(())
}
fn prepare(source: &Path, destination: &Path) -> Result<()> {
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(source.join("anime/manifest.json"))?)?;
    anyhow::ensure!(
        manifest["name"] == "anime",
        "App model resources must use anime"
    );
    let mut entries = Vec::new();
    for (folder, names) in REQUIRED {
        let directory = source.join(folder);
        for path in files(&directory)? {
            let name = path.strip_prefix(&directory)?.to_string_lossy();
            if names.contains(&name.as_ref())
                || names
                    .iter()
                    .any(|n| n.ends_with(".onnx") && name.starts_with(&format!("{n}.")))
            {
                let (size, sha256) = fingerprint(&path)?;
                entries.push(Entry {
                    path: format!("{folder}/{name}"),
                    size,
                    sha256,
                });
            }
        }
    }
    let paths = validate_files(&entries)?;
    // Reject unsupported destination entries before copying any data.
    files(destination)?;
    let (source, destination) = separate_directories(source, destination)?;
    for entry in &entries {
        let target = destination.join(&entry.path);
        if target.is_file() && fingerprint(&target)? == (entry.size, entry.sha256.clone()) {
            continue;
        }
        let parent = target.parent().unwrap();
        fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        anyhow::ensure!(
            copy_hashed(&source.join(&entry.path), &mut temporary)?
                == (entry.size, entry.sha256.clone()),
            "Model changed while copying: {}",
            entry.path
        );
        temporary.persist(&target)?;
    }
    let expected: BTreeSet<_> = paths
        .into_iter()
        .chain(["bundle.json"])
        .map(|p| destination.join(p))
        .collect();
    for path in files(&destination)?.difference(&expected) {
        fs::remove_file(path)?;
    }
    let index = Index {
        model: "anime".into(),
        checkpoint: manifest["checkpoint"].clone(),
        files: entries,
    };
    json_write(&destination.join("bundle.json"), &index)?;
    println!(
        "Bundled anime + codec: {} bytes ({} files)",
        index.files.iter().map(|e| e.size).sum::<u64>(),
        index.files.len()
    );
    Ok(())
}

pub fn cli(args: &[String]) -> Result<()> {
    let command = args.first().map(String::as_str);
    let distributing = matches!(command, Some("distribute" | "publish"));
    let bundle = "pkgs/app/resources/models";
    let source = PathBuf::from(
        option(args, "--source")?
            .unwrap_or_else(|| if distributing { bundle } else { "models" }.into()),
    );
    let destination = PathBuf::from(
        option(args, "--destination")?
            .unwrap_or_else(|| if distributing { "model-assets" } else { bundle }.into()),
    );
    match command {
        Some("prepare") => prepare(&source, &destination),
        Some("check") => check(&destination),
        Some("distribute") => {
            distribution::distribute(&source, &destination, option(args, "--repository")?)
                .map(|_| ())
        }
        Some("publish") => distribution::publish(&destination, |args, directory| {
            let result = Command::new("oras")
                .args(args)
                .current_dir(directory)
                .stderr(Stdio::inherit())
                .output()
                .context("Run oras")?;
            anyhow::ensure!(result.status.success(), "oras failed: {}", result.status);
            Ok(result.stdout)
        })
        .map(|_| ()),
        Some("export") => run(
            "uv",
            &[
                "run",
                "--project",
                "pkgs/engine/script",
                "python",
                "pkgs/engine/script/export_onnx.py",
                "--out",
                "models",
                "--models",
                "anime",
            ],
        ),
        _ => bail!(
            "Usage: cargo xtask models prepare|check|export|distribute|publish [--source DIR] [--destination DIR] [--repository OWNER/PACKAGE]"
        ),
    }
}
