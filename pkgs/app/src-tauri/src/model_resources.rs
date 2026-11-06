use anyhow::{Result, ensure};
use irodoru_engine::output::sha256;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

#[derive(Deserialize)]
struct Bundle {
    model: String,
    files: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    path: String,
    size: u64,
    sha256: String,
}

pub fn materialize(
    index: &[u8],
    root: &Path,
    mut open: impl FnMut(&Path) -> Result<std::fs::File>,
) -> Result<PathBuf> {
    let bundle: Bundle = serde_json::from_slice(index)?;
    ensure!(
        bundle.model == "anime" && !bundle.files.is_empty(),
        "invalid bundled model"
    );
    let version = sha256(index);
    let destination = root.join(version);
    for entry in &bundle.files {
        let path = Path::new(&entry.path);
        ensure!(
            path.components().all(|c| matches!(c, Component::Normal(_)))
                && (path.starts_with("anime") || path.starts_with("codec")),
            "invalid model resource path"
        );
    }
    let complete = destination.join(".complete");
    if complete.is_file()
        && bundle.files.iter().all(|entry| {
            destination
                .join(&entry.path)
                .metadata()
                .is_ok_and(|m| m.len() == entry.size)
        })
    {
        return Ok(destination);
    }
    if complete.exists() {
        std::fs::remove_file(&complete)?;
    }
    std::fs::create_dir_all(&destination)?;
    for entry in bundle.files {
        let target = destination.join(&entry.path);
        std::fs::create_dir_all(target.parent().unwrap())?;
        let temporary = target.with_extension("partial");
        let mut input = open(Path::new(&entry.path))?;
        let mut output = std::fs::File::create(&temporary)?;
        let mut hash = Sha256::new();
        let mut size = 0u64;
        let mut buffer = vec![0; 1024 * 1024];
        loop {
            let n = input.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            size += n as u64;
            ensure!(size <= entry.size, "model resource is too large");
            hash.update(&buffer[..n]);
            output.write_all(&buffer[..n])?;
        }
        let checksum: String = hash
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        ensure!(
            size == entry.size && checksum == entry.sha256,
            "model resource checksum mismatch: {}",
            entry.path
        );
        output.sync_all()?;
        drop(output);
        std::fs::rename(temporary, target)?;
    }
    std::fs::write(complete, [])?;
    Ok(destination)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verifies_extraction_reuses_complete_bundle_and_rejects_corruption() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        std::fs::write(&source, b"model").unwrap();
        let index = serde_json::to_vec(&serde_json::json!({"model":"anime", "files":[{
            "path":"anime/backbone.onnx", "size":5, "sha256":sha256(b"model")
        }]}))
        .unwrap();
        let destination = materialize(&index, &root.path().join("models"), |_| {
            Ok(std::fs::File::open(&source)?)
        })
        .unwrap();
        assert_eq!(
            std::fs::read(destination.join("anime/backbone.onnx")).unwrap(),
            b"model"
        );
        materialize(&index, &root.path().join("models"), |_| {
            panic!("already extracted")
        })
        .unwrap();
        std::fs::write(&source, b"wrong").unwrap();
        assert!(
            materialize(&index, &root.path().join("corrupt"), |_| Ok(
                std::fs::File::open(&source)?
            ))
            .is_err()
        );
        let bad = String::from_utf8(index)
            .unwrap()
            .replace("anime/backbone.onnx", "anime/../../escape");
        assert!(materialize(bad.as_bytes(), root.path(), |_| panic!("invalid path")).is_err());
    }
}
