use super::Session;
use anyhow::{Context, Result, bail};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

pub(super) struct Snapshot {
    pub(super) id: String,
    pub(super) revision: u64,
    pub(super) path: PathBuf,
    pub(super) files: Vec<(String, PathBuf)>,
    _directory: tempfile::TempDir,
}
pub(super) fn snapshot(session: &Session) -> Result<Snapshot> {
    fn visit(
        root: &Path,
        path: &Path,
        staging: &Path,
        files: &mut Vec<(String, PathBuf)>,
    ) -> Result<()> {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                visit(root, &path, staging, files)?;
            } else if entry.file_type()?.is_file() {
                let relative = path.strip_prefix(root)?;
                let target = staging.join(relative);
                std::fs::create_dir_all(target.parent().unwrap())?;
                // Working files are replaced atomically, never modified in place. Hard links
                // pin their current contents without holding Windows handles to editable paths.
                // Copy on filesystems without hard-link support.
                if std::fs::hard_link(&path, &target).is_err() {
                    std::fs::copy(&path, &target)?;
                }
                files.push((relative.to_string_lossy().replace('\\', "/"), target));
            } else {
                bail!("プロジェクトに通常ファイル以外が含まれています");
            }
        }
        Ok(())
    }
    let directory = tempfile::Builder::new()
        .prefix(".snapshot-")
        .tempdir_in(session.library.root())?;
    let mut files = Vec::new();
    visit(
        session.library.root(),
        session.library.root(),
        directory.path(),
        &mut files,
    )?;
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(Snapshot {
        id: session.id.clone(),
        revision: session.revision,
        path: session.path.clone(),
        files,
        _directory: directory,
    })
}
pub(super) fn write_archive(snapshot: &Snapshot) -> Result<()> {
    let parent = snapshot.path.parent().context("保存先が不正です")?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    let mut zip = ZipWriter::new(temp.as_file_mut());
    for (name, source) in &snapshot.files {
        let method = if name.ends_with(".yaml") {
            zip::CompressionMethod::Deflated
        } else {
            zip::CompressionMethod::Stored
        };
        zip.start_file(
            name,
            SimpleFileOptions::default().compression_method(method),
        )?;
        std::io::copy(&mut File::open(source)?, &mut zip)?;
    }
    zip.finish()?.sync_all()?;
    temp.persist(&snapshot.path).map_err(|e| e.error)?;
    Ok(())
}

pub(super) fn extract(file: File, root: &Path) -> Result<()> {
    let mut zip = ZipArchive::new(file)?;
    if zip.len() > 100_000 {
        bail!("プロジェクトのファイル数が多すぎます");
    }
    let mut seen = std::collections::HashSet::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name()?.into_owned();
        let path = Path::new(&name);
        if name.contains(['\\', ':'])
            || path
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
            || entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            bail!("ZIP 内のパスが不正です");
        }
        if entry.is_dir() {
            continue;
        }
        if !seen.insert(name.clone()) {
            bail!("ZIP 内のファイルが重複しています");
        }
        let parts: Vec<_> = name.split('/').collect();
        let allowed = match parts.as_slice() {
            ["project.yaml"] => true,
            ["scripts", file] => file
                .strip_suffix(".yaml")
                .is_some_and(crate::library::valid_script_id),
            [
                "characters",
                id,
                "character.yaml" | "icon.png" | "preview.wav",
            ] => crate::domain::valid_id(id),
            ["voices", file] => file
                .strip_suffix(".wav")
                .is_some_and(crate::domain::valid_id),
            _ => false,
        };
        if !allowed {
            bail!("未対応のプロジェクトファイル: {name}");
        }
        const LIMIT: u64 = 4 * 1024 * 1024 * 1024;
        let limit = if name.ends_with(".yaml") || name.ends_with(".png") {
            4 * 1024 * 1024
        } else {
            LIMIT - total
        };
        if entry.size() > limit || entry.size() > LIMIT - total {
            bail!("プロジェクトが大きすぎます");
        }
        let destination = root.join(path);
        std::fs::create_dir_all(destination.parent().unwrap())?;
        let mut output = File::create(destination)?;
        let count = std::io::copy(
            &mut entry.by_ref().take(limit.min(LIMIT - total) + 1),
            &mut output,
        )?;
        total += count;
        if count > limit || total > LIMIT {
            bail!("プロジェクトが大きすぎます");
        }
        output.flush()?;
    }
    if !seen.contains("project.yaml") {
        bail!("project.yaml がありません");
    }
    Ok(())
}
