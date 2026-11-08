use super::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, io::Read};
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
#[derive(Serialize, Deserialize)]
struct Entry {
    path: String,
    size: u64,
    sha256: String,
}
#[derive(Serialize, Deserialize)]
struct Index {
    model: String,
    checkpoint: serde_json::Value,
    files: Vec<Entry>,
}
pub fn fingerprint(path: &Path) -> Result<(u64, String)> {
    let mut input = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut size = 0;
    let mut buffer = [0u8; 65536];
    loop {
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
        size += n as u64;
    }
    Ok((size, format!("{:x}", hash.finalize())))
}
pub fn files(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut result = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            result.extend(files(&entry.path())?);
        } else if kind.is_file() {
            result.push(entry.path());
        } else {
            bail!("Unsupported resource: {}", entry.path().display());
        }
    }
    result.sort();
    Ok(result)
}
fn valid_path(path: &str) -> bool {
    let parts: Vec<_> = path.split('/').collect();
    parts.len() == 2
        && ["anime", "codec"].contains(&parts[0])
        && !["", ".", ".."].contains(&parts[1])
        && !path.contains(['\\', ':'])
}
pub fn check(destination: &Path) -> Result<()> {
    let index: Index = serde_json::from_slice(&fs::read(destination.join("bundle.json"))?)?;
    anyhow::ensure!(index.model == "anime", "The app must bundle anime");
    let paths: BTreeSet<_> = index.files.iter().map(|e| e.path.as_str()).collect();
    anyhow::ensure!(
        paths.len() == index.files.len(),
        "Duplicate model resources"
    );
    for (dir, names) in REQUIRED {
        for name in *names {
            anyhow::ensure!(
                paths.contains(format!("{dir}/{name}").as_str()),
                "Missing {dir}/{name}"
            );
        }
    }
    for entry in &index.files {
        anyhow::ensure!(valid_path(&entry.path), "Invalid model resource path");
        let actual = fingerprint(&destination.join(&entry.path))?;
        anyhow::ensure!(
            actual == (entry.size, entry.sha256.clone()),
            "Model checksum mismatch: {}",
            entry.path
        );
    }
    anyhow::ensure!(
        files(destination)?.len() == paths.len() + 1,
        "Unexpected model resources; run cargo xtask models prepare"
    );
    println!("Anime model resources verified");
    Ok(())
}
pub fn prepare(source: &Path, destination: &Path) -> Result<()> {
    let source = source.canonicalize()?;
    for (dir, names) in REQUIRED {
        for name in *names {
            let path = source.join(dir).join(name);
            let metadata = fs::metadata(&path).with_context(|| {
                format!("Missing {}. Run cargo xtask models export", path.display())
            })?;
            anyhow::ensure!(
                metadata.is_file() && metadata.len() > 0,
                "Empty model: {}",
                path.display()
            );
        }
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(source.join("anime/manifest.json"))?)?;
    anyhow::ensure!(manifest["name"] == "anime", "The app must bundle anime");
    fs::create_dir_all(destination)?;
    let destination = destination.canonicalize()?;
    anyhow::ensure!(
        !source.starts_with(&destination) && !destination.starts_with(&source),
        "Model staging directories must not overlap"
    );
    let mut entries = Vec::new();
    for (dir, names) in REQUIRED {
        for path in files(&source.join(dir))? {
            let name = path.file_name().unwrap().to_string_lossy();
            if !names.contains(&name.as_ref())
                && !names
                    .iter()
                    .any(|n| n.ends_with(".onnx") && name.starts_with(&format!("{n}.")))
            {
                continue;
            }
            let relative = format!("{dir}/{name}");
            let target = destination.join(&relative);
            fs::create_dir_all(target.parent().unwrap())?;
            let (size, sha256) = fingerprint(&path)?;
            if !target.is_file() || fingerprint(&target)? != (size, sha256.clone()) {
                let temp = target.with_extension("copy-tmp");
                fs::copy(&path, &temp)?;
                if cfg!(windows) && target.exists() {
                    fs::remove_file(&target)?;
                }
                fs::rename(temp, &target)?;
            }
            entries.push(Entry {
                path: relative,
                size,
                sha256,
            });
        }
    }
    let expected: BTreeSet<_> = entries
        .iter()
        .map(|e| destination.join(&e.path))
        .chain([destination.join("bundle.json")])
        .collect();
    for path in files(&destination)? {
        if !expected.contains(&path) {
            fs::remove_file(path)?;
        }
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
    if matches!(
        args.first().map(String::as_str),
        Some("distribute" | "publish")
    ) {
        let status = Command::new(if cfg!(windows) { "python" } else { "python3" })
            .arg(root().join("scripts/model_distribution.py"))
            .args(args)
            .status()
            .context("Run model distribution helper (requires Python 3.10+)")?;
        anyhow::ensure!(status.success(), "Model distribution failed: {status}");
        return Ok(());
    }
    let source = PathBuf::from(option(args, "--source")?.unwrap_or_else(|| "models".into()));
    let destination = PathBuf::from(
        option(args, "--destination")?.unwrap_or_else(|| "pkgs/app/resources/models".into()),
    );
    match args.first().map(String::as_str) {
        Some("prepare") => prepare(&source, &destination),
        Some("check") => check(&destination),
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
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(dir: &Path) {
        for (folder, names) in REQUIRED {
            fs::create_dir_all(dir.join(folder)).unwrap();
            for name in *names {
                fs::write(dir.join(folder).join(name), b"model").unwrap();
            }
        }
        fs::write(dir.join("anime/manifest.json"), r#"{"name":"anime"}"#).unwrap();
    }
    #[test]
    fn stage_checks_external_data_and_rejects_corruption() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("source");
        let dst = temp.path().join("dest");
        fixture(&src);
        fs::write(src.join("anime/backbone.onnx.data"), b"weights").unwrap();
        prepare(&src, &dst).unwrap();
        check(&dst).unwrap();
        fs::write(dst.join("anime/backbone.onnx.data"), b"corrupt").unwrap();
        assert!(check(&dst).is_err());
        prepare(&src, &dst).unwrap();
        fs::write(dst.join("extra"), "stale").unwrap();
        assert!(check(&dst).is_err());
        prepare(&src, &dst).unwrap();
        check(&dst).unwrap();
        assert!(prepare(&src, &src).is_err());
    }
    #[test]
    fn resource_paths_cannot_escape_bundle() {
        for p in [
            "../secret",
            "anime/../../x",
            "anime/..",
            "anime/C:x",
            "anime/\\x",
        ] {
            assert!(!valid_path(p));
        }
    }
}
