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
    let dirs = crate::files::project_dirs().context("モデルの保存フォルダが見つかりません")?;
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
