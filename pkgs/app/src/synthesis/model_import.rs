//! Local ZIPs are installed independently of the original file and of downloaded models.
use crate::domain::InferenceStatus;
use anyhow::{Context, Result, ensure};
use irodoru_model_bundle::{MAX_ARCHIVE_SIZE as MAX_BYTES, required_paths};
use std::{collections::HashSet, fs::File, io::Read, path::Path};
use tempfile::TempDir;
use zip::{CompressionMethod, ZipArchive};

const MAX_ENTRIES: usize = 256;
const MAX_METADATA: u64 = 1024 * 1024;

pub(super) fn import(path: &Path, progress: &dyn Fn(InferenceStatus)) -> Result<TempDir> {
    let dirs = directories::ProjectDirs::from("io.github", "kanade-k-1228", "irodoru")
        .context("モデルの保存フォルダが見つかりません")?;
    extract(
        path,
        &dirs.data_local_dir().join("imported-models"),
        progress,
    )
    .context("モデルZIPを取り込めませんでした")
}

fn allowed_file(name: &str) -> bool {
    required_paths().any(|required| {
        name == required
            || required.ends_with(".onnx")
                && name.strip_prefix(required.as_str()).is_some_and(|suffix| {
                    suffix.starts_with('.')
                        && suffix.len() > 1
                        && suffix
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                })
    })
}

// Keep the temporary directory owned by the caller until both engine loading and
// preference persistence succeed. Any earlier error removes the entire import.
fn extract(path: &Path, root: &Path, progress: &dyn Fn(InferenceStatus)) -> Result<TempDir> {
    let file = File::open(path).context("ZIPファイルを開けません")?;
    ensure!(
        file.metadata()?.len() <= MAX_BYTES,
        "モデルZIPが大きすぎます"
    );
    let mut archive = ZipArchive::new(file).context("有効なZIPファイルではありません")?;
    ensure!(
        archive.len() <= MAX_ENTRIES,
        "モデルZIPのファイル数が多すぎます"
    );
    ensure!(
        !archive.has_duplicate_names(),
        "モデルZIPに重複したパスがあります"
    );
    let mut paths = HashSet::new();
    let mut total = 0u64;
    // Inspect every entry before creating files. Never join an unchecked ZIP path.
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        let name = entry.name()?.into_owned();
        ensure!(
            paths.insert(name.clone()),
            "モデルZIPに重複したパスがあります"
        );
        if entry.is_dir() {
            ensure!(
                matches!(name.as_str(), "anime/" | "codec/")
                    && entry.size() == 0
                    && entry
                        .unix_mode()
                        .is_none_or(|mode| matches!(mode & 0o170000, 0 | 0o040000)),
                "モデルZIPに不正なフォルダがあります: {name}"
            );
            continue;
        }
        ensure!(
            allowed_file(&name),
            "モデルZIPに未対応のファイルがあります: {name}"
        );
        if matches!(name.as_str(), "anime/manifest.json" | "codec/codec.json") {
            ensure!(
                entry.size() <= MAX_METADATA,
                "モデル設定ファイルが大きすぎます"
            );
        }
        ensure!(
            entry
                .unix_mode()
                .is_none_or(|mode| matches!(mode & 0o170000, 0 | 0o100000))
                && matches!(
                    entry.compression(),
                    CompressionMethod::Stored | CompressionMethod::Deflated
                )
                && entry.size() > 0,
            "モデルZIPに不正なファイルがあります: {name}"
        );
        total = total
            .checked_add(entry.size())
            .context("モデルのサイズが不正です")?;
        ensure!(total <= MAX_BYTES, "展開後のモデルが大きすぎます");
    }
    ensure!(
        required_paths().all(|name| paths.contains(&name)),
        "モデルZIPに必要なファイルが不足しています。anime と codec を含むモデルZIPを選択してください"
    );

    std::fs::create_dir_all(root).context("モデルの保存フォルダを作成できません")?;
    let staging = tempfile::Builder::new().prefix("model-").tempdir_in(root)?;
    let mut extracted = 0;
    progress(InferenceStatus::Extracting { extracted, total });
    let mut buffer = vec![0; 1024 * 1024];
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let target = staging.path().join(entry.name()?.as_ref());
        std::fs::create_dir_all(target.parent().unwrap())?;
        let mut output = File::create(target)?;
        let mut written = 0u64;
        loop {
            // Reading to EOF also verifies the ZIP CRC. Bound actual output even
            // when a corrupt archive lies about its uncompressed size.
            let n = entry
                .read(&mut buffer)
                .context("ZIPが破損しているか、読み込みが中断されました")?;
            if n == 0 {
                break;
            }
            written += n as u64;
            ensure!(written <= entry.size(), "モデルファイルのサイズが不正です");
            std::io::Write::write_all(&mut output, &buffer[..n])
                .context("モデルを保存できません。空き容量を確認してください")?;
            extracted += n as u64;
            progress(InferenceStatus::Extracting { extracted, total });
        }
        ensure!(
            written == entry.size(),
            "モデルファイルが途中で切れています"
        );
        output.sync_all()?;
    }
    validate_metadata(staging.path())?;
    Ok(staging)
}

fn metadata(path: &Path) -> Result<serde_json::Value> {
    let file = File::open(path)?;
    ensure!(
        file.metadata()?.len() <= MAX_METADATA,
        "モデル設定ファイルが大きすぎます"
    );
    Ok(serde_json::from_reader(file.take(MAX_METADATA + 1))?)
}

fn validate_metadata(root: &Path) -> Result<()> {
    let model = metadata(&root.join("anime/manifest.json"))?;
    ensure!(
        model["name"] == "anime"
            && model["checkpoint"] == "phasefield-audio/Irodori-TTS-v4.1-Anime",
        "このZIPは対応するanimeモデルではありません"
    );
    let codec = metadata(&root.join("codec/codec.json"))?;
    ensure!(
        ["sample_rate", "hop_length", "latent_dim"]
            .iter()
            .all(|key| codec[key].as_u64().is_some_and(|v| v > 0)),
        "コーデックの設定が不正です"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::RefCell,
        io::{Cursor, Write},
    };
    use zip::{ZipWriter, write::SimpleFileOptions};

    fn content(name: &str) -> &'static [u8] {
        match name {
            "anime/manifest.json" => {
                br#"{"name":"anime","checkpoint":"phasefield-audio/Irodori-TTS-v4.1-Anime"}"#
            }
            "codec/codec.json" => br#"{"sample_rate":24000,"hop_length":1920,"latent_dim":32}"#,
            _ => b"model-bytes",
        }
    }

    fn archive(
        method: CompressionMethod,
        omit: Option<&str>,
        extra: impl FnOnce(&mut ZipWriter<Cursor<Vec<u8>>>),
    ) -> Vec<u8> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(method);
        for name in required_paths().filter(|name| Some(name.as_str()) != omit) {
            zip.start_file(&name, options).unwrap();
            zip.write_all(content(&name)).unwrap();
        }
        extra(&mut zip);
        zip.finish().unwrap().into_inner()
    }

    fn assert_rejected(bytes: &[u8]) {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("models.zip");
        std::fs::write(&source, bytes).unwrap();
        let root = temp.path().join("imports");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("previous-model"), b"preserve").unwrap();
        assert!(extract(&source, &root, &|_| {}).is_err());
        assert_eq!(
            std::fs::read_dir(&root).unwrap().count(),
            1,
            "no partial import may remain"
        );
        assert_eq!(
            std::fs::read(root.join("previous-model")).unwrap(),
            b"preserve"
        );
    }

    #[test]
    fn stored_and_deflated_imports_survive_removing_the_original_zip() {
        for method in [CompressionMethod::Stored, CompressionMethod::Deflated] {
            let temp = tempfile::tempdir().unwrap();
            let source = temp.path().join("日本語モデル.zip");
            let bytes = archive(method, None, |zip| {
                zip.add_directory("anime/", SimpleFileOptions::default())
                    .unwrap();
                zip.add_directory("codec/", SimpleFileOptions::default())
                    .unwrap();
                zip.start_file("anime/backbone.onnx.data", SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(b"weights").unwrap();
            });
            std::fs::write(&source, bytes).unwrap();
            let updates = RefCell::new(Vec::new());
            let imported = extract(&source, &temp.path().join("imports"), &|status| {
                if let InferenceStatus::Extracting { extracted, total } = status {
                    updates.borrow_mut().push((extracted, total));
                }
            })
            .unwrap()
            .keep();
            std::fs::remove_file(&source).unwrap();
            for name in required_paths() {
                assert_eq!(std::fs::read(imported.join(&name)).unwrap(), content(&name));
            }
            assert_eq!(
                std::fs::read(imported.join("anime/backbone.onnx.data")).unwrap(),
                b"weights"
            );
            let updates = updates.borrow();
            assert_eq!(updates.first().unwrap().0, 0);
            assert_eq!(updates.last().unwrap().0, updates.last().unwrap().1);
            assert!(updates.windows(2).all(|pair| pair[0].0 <= pair[1].0));
        }
    }

    #[test]
    fn rejects_missing_files_unsafe_paths_and_links_before_installation() {
        assert_rejected(&archive(
            CompressionMethod::Stored,
            Some("codec/dacvae_decoder.onnx"),
            |_| {},
        ));
        for name in [
            "../escape",
            "/escape",
            "anime/../escape",
            "anime\\escape",
            "anime/extra.onnx",
            "anime/backbone.onnx.data/escape",
        ] {
            assert_rejected(&archive(CompressionMethod::Stored, None, |zip| {
                zip.start_file(name, SimpleFileOptions::default()).unwrap();
                zip.write_all(b"bad").unwrap();
            }));
        }
        assert_rejected(&archive(
            CompressionMethod::Stored,
            Some("anime/backbone.onnx"),
            |zip| {
                zip.add_symlink(
                    "anime/backbone.onnx",
                    "../../outside",
                    SimpleFileOptions::default(),
                )
                .unwrap();
            },
        ));
    }

    #[test]
    fn rejects_duplicate_names_even_when_the_zip_library_deduplicates_entries() {
        let mut bytes = archive(CompressionMethod::Stored, None, |zip| {
            zip.start_file("anime/manifest.jsoX", SimpleFileOptions::default())
                .unwrap();
            zip.write_all(content("anime/manifest.json")).unwrap();
        });
        let name = b"anime/manifest.jsoX";
        for offset in 0..=bytes.len() - name.len() {
            if &bytes[offset..offset + name.len()] == name {
                bytes[offset + name.len() - 1] = b'n';
            }
        }
        assert_rejected(&bytes);
    }

    #[test]
    fn corrupt_and_truncated_archives_leave_no_partial_models() {
        let mut bytes = archive(CompressionMethod::Stored, None, |_| {});
        let position = bytes
            .windows(b"model-bytes".len())
            .position(|s| s == b"model-bytes")
            .unwrap();
        bytes[position] ^= 1;
        assert_rejected(&bytes);
        let bytes = archive(CompressionMethod::Stored, None, |_| {});
        assert_rejected(&bytes[..bytes.len() / 2]);
    }

    #[test]
    fn rejects_incompatible_model_and_invalid_codec_metadata() {
        for (name, bytes) in [
            (
                "anime/manifest.json",
                br#"{"name":"other","checkpoint":"other"}"#.as_slice(),
            ),
            (
                "codec/codec.json",
                br#"{"sample_rate":0,"hop_length":1920,"latent_dim":32}"#.as_slice(),
            ),
            ("codec/codec.json", b"not json".as_slice()),
        ] {
            assert_rejected(&archive(CompressionMethod::Stored, Some(name), |zip| {
                zip.start_file(name, SimpleFileOptions::default()).unwrap();
                zip.write_all(bytes).unwrap();
            }));
        }
    }

    #[test]
    fn rejects_excessive_declared_output_size_and_metadata() {
        let mut bytes = archive(CompressionMethod::Stored, None, |_| {});
        // ZIP32 central directory: uncompressed length is at offset 24.
        for i in 0..bytes.len() - 46 {
            if &bytes[i..i + 4] == b"PK\x01\x02" {
                let name_len = u16::from_le_bytes([bytes[i + 28], bytes[i + 29]]) as usize;
                if bytes[i + 46..i + 46 + name_len].ends_with(b".onnx") {
                    bytes[i + 24..i + 28].copy_from_slice(&u32::MAX.to_le_bytes());
                }
            }
        }
        assert_rejected(&bytes);
        assert_rejected(&archive(
            CompressionMethod::Deflated,
            Some("anime/manifest.json"),
            |zip| {
                zip.start_file(
                    "anime/manifest.json",
                    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
                )
                .unwrap();
                zip.write_all(&vec![b' '; MAX_METADATA as usize + 1])
                    .unwrap();
            },
        ));
    }

    #[test]
    fn abandoning_an_import_removes_staged_files() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("models.zip");
        std::fs::write(&source, archive(CompressionMethod::Stored, None, |_| {})).unwrap();
        let imported = extract(&source, &temp.path().join("imports"), &|_| {}).unwrap();
        let path = imported.path().to_owned();
        assert!(path.exists());
        drop(imported);
        assert!(!path.exists());
    }
}
