//! Generate the same complete notice bundle for desktop and container releases.
use crate::{option, output, root};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::{
    fs,
    path::{Path, PathBuf},
};

const ABOUT_VERSION: &str = "cargo-about 0.9.3";

#[derive(Debug, Deserialize)]
struct Package {
    name: String,
    version: String,
    source: Option<String>,
    manifest_path: PathBuf,
    license: Option<String>,
    #[serde(default)]
    authors: Vec<String>,
}
#[derive(Deserialize)]
struct UsedBy {
    #[serde(rename = "crate")]
    package: Package,
}
#[derive(Deserialize)]
struct License {
    id: String,
    text: String,
    source_path: Option<PathBuf>,
    used_by: Vec<UsedBy>,
}
#[derive(Deserialize)]
struct Report {
    licenses: Vec<License>,
}

fn notice_files(directory: &Path, result: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            notice_files(&entry.path(), result)?;
        } else if kind.is_file() {
            if entry
                .path()
                .extension()
                .is_some_and(|ext| ["rs", "c", "cpp", "h"].iter().any(|e| ext == *e))
            {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_ascii_uppercase();
            if [
                "LICENSE",
                "LICENCE",
                "COPYING",
                "COPYRIGHT",
                "NOTICE",
                "THIRDPARTYNOTICES",
                "FTL.TXT",
            ]
            .iter()
            .any(|prefix| {
                name == *prefix
                    || name.starts_with(&format!("{prefix}."))
                    || name.starts_with(&format!("{prefix}-"))
            }) {
                result.push(entry.path());
            }
        }
    }
    Ok(())
}

fn append_file(text: &mut String, path: &Path, label: &str) -> Result<()> {
    let bytes = fs::read(path).with_context(|| format!("Read notice {}", path.display()))?;
    anyhow::ensure!(!bytes.is_empty(), "Empty notice: {}", path.display());
    text.push_str(&format!("\n\n--- {label} ---\n"));
    text.push_str(&String::from_utf8_lossy(&bytes));
    text.push('\n');
    Ok(())
}

fn render(report: &Report, destination: &Path) -> Result<()> {
    anyhow::ensure!(
        !report.licenses.is_empty(),
        "Empty third-party license report"
    );
    let mut notices = String::from(
        "irodoru third-party licenses\n\n\
        This bundle covers all supported desktop/server targets and includes build\n\
        dependencies. Some components are absent from a particular installation.\n\
        The application LICENSE does not replace these terms. Selected SPDX licenses\n\
        are listed below; original notices may also describe alternative licenses.\n\
        MPL source archives and exact source URLs are listed in THIRD-PARTY-SOURCES.txt.\n",
    );
    let mut packages = BTreeMap::new();
    let clarifications: BTreeMap<String, String> =
        serde_json::from_slice(&fs::read(root().join("licenses/clarifications.json"))?)?;
    for license in &report.licenses {
        anyhow::ensure!(
            !license.text.trim().is_empty(),
            "Missing {} license text",
            license.id
        );
        notices.push_str(&format!("\n\n=== {} ===\n", license.id));
        for used in &license.used_by {
            let package = &used.package;
            notices.push_str(&format!("{} {}\n", package.name, package.version));
            packages.insert(format!("{}-{}", package.name, package.version), package);
            if package.source.is_some()
                && license.source_path.is_none()
                && matches!(
                    license.id.as_str(),
                    "MIT" | "BSD-2-Clause" | "BSD-3-Clause" | "ISC"
                )
            {
                let key = format!("{}-{}", package.name, package.version);
                let mut local = Vec::new();
                notice_files(
                    package
                        .manifest_path
                        .parent()
                        .context("Missing crate directory")?,
                    &mut local,
                )?;
                anyhow::ensure!(
                    !local.is_empty() || clarifications.contains_key(&key),
                    "Missing original copyright notice for {key}; review and add a pinned clarification"
                );
            }
        }
        if license.source_path.is_none() {
            notices.push_str(
                "\nSPDX reference text; original author/copyright notices follow per component.\n",
            );
        }
        notices.push_str(&license.text);
    }
    fs::create_dir_all(destination.join("sources"))?;
    let mut sources = String::from(
        "Third-party source availability\n\n\
        MPL-2.0 components, including Symphonia and option-ext, remain under MPL-2.0.\n\
        Their unmodified published source archives are included in sources/ alongside\n\
        this file. Extract .crate files as gzip-compressed tar archives. The included\n\
        license texts and notices apply to that source. No restrictions in irodoru's\n\
        license alter your rights to the covered source. Exact upstream archives for\n\
        all registry dependencies are also linked below.\n\n",
    );
    let mut native_checked = false;
    for (key, package) in packages {
        if package.source.is_none() {
            continue;
        }
        anyhow::ensure!(
            package.source.as_deref()
                == Some("registry+https://github.com/rust-lang/crates.io-index"),
            "Add source availability handling for {key}: {:?}",
            package.source
        );
        let directory = package
            .manifest_path
            .parent()
            .context("Missing crate directory")?;
        notices.push_str(&format!(
            "\n\n{key}\nAuthors as published in Cargo.toml: {}\n",
            package.authors.join(", ")
        ));
        if let Some(clarification) = clarifications.get(&key) {
            append_file(
                &mut notices,
                &root().join("licenses/clarifications").join(clarification),
                &key,
            )?;
        }
        sources.push_str(&format!(
            "{key} ({})\nhttps://crates.io/api/v1/crates/{}/{}/download\n",
            package
                .license
                .as_deref()
                .context("Missing crate license")?,
            package.name,
            package.version
        ));
        if package.license.as_deref() == Some("MPL-2.0") {
            let index = directory.parent().context("Missing registry index")?;
            let registry = index
                .parent()
                .and_then(Path::parent)
                .context("Missing registry root")?;
            let archive = registry
                .join("cache")
                .join(index.file_name().unwrap())
                .join(format!("{key}.crate"));
            fs::copy(
                &archive,
                destination.join("sources").join(format!("{key}.crate")),
            )
            .with_context(|| {
                format!(
                    "Include MPL source {}; run cargo fetch --locked",
                    archive.display()
                )
            })?;
            sources.push_str(&format!("Included source: sources/{key}.crate\n"));
        }
        sources.push('\n');
        // Crate metadata describes Rust bindings, not necessarily vendored C/C++.
        // Preserve original LICENSE/COPYING/NOTICE files too (e.g. FreeType, ring).
        let mut files = Vec::new();
        notice_files(directory, &mut files)?;
        files.sort();
        for path in files {
            append_file(
                &mut notices,
                &path,
                &format!("{key}/{}", path.strip_prefix(directory)?.display()),
            )?;
        }
        if package.name == "ort-sys" {
            anyhow::ensure!(
                package.version == "2.0.0-rc.15",
                "Review native ONNX Runtime notices after updating ort-sys"
            );
            let table = fs::read_to_string(directory.join("build/download/dist.tsv"))?;
            anyhow::ensure!(
                table
                    .lines()
                    .filter(|line| !line.is_empty())
                    .skip(1)
                    .all(|line| line.contains("ms@1.31.0/")),
                "Review ONNX Runtime native version"
            );
            native_checked = true;
        }
    }
    anyhow::ensure!(
        native_checked,
        "ONNX Runtime license version was not checked"
    );
    append_file(
        &mut notices,
        &root().join("pkgs/app/icons/ui/LICENSE"),
        "Lucide / Feather icons",
    )?;
    let native = root().join("licenses/native");
    for name in [
        "NOTICE.txt",
        "onnxruntime-1.31.0-LICENSE.txt",
        "onnxruntime-1.31.0-ThirdPartyNotices.txt",
        "onnxruntime-1.31.0-sources.txt",
        "directml-1.15.4-LICENSE.txt",
        "directml-1.15.4-ThirdPartyNotices.txt",
    ] {
        anyhow::ensure!(
            native.join(name).is_file(),
            "Missing native runtime notice {name}"
        );
    }
    let mut paths: Vec<_> = fs::read_dir(&native)?
        .map(|e| e.map(|e| e.path()))
        .collect::<std::io::Result<_>>()?;
    paths.sort();
    for path in paths {
        if path.extension().is_some_and(|ext| ext == "txt") {
            append_file(
                &mut notices,
                &path,
                path.file_name()
                    .unwrap()
                    .to_str()
                    .context("Invalid notice name")?,
            )?;
        }
    }
    fs::copy(root().join("LICENSE"), destination.join("LICENSE"))?;
    fs::write(destination.join("THIRD-PARTY-NOTICES.txt"), notices)?;
    fs::write(destination.join("THIRD-PARTY-SOURCES.txt"), sources)?;
    fs::write(destination.join(".irodoru-license-bundle"), "1\n")?;
    let model = root().join("licenses/model");
    fs::create_dir_all(destination.join("model"))?;
    for entry in fs::read_dir(model)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            fs::copy(
                entry.path(),
                destination.join("model").join(entry.file_name()),
            )?;
        }
    }
    Ok(())
}

pub(super) fn generate(destination: &Path) -> Result<()> {
    validate_destination(destination)?;
    let version = output("cargo", &["about", "--version"]).context(
        "Install cargo-about: cargo install cargo-about --version 0.9.3 --locked --features cli",
    )?;
    anyhow::ensure!(
        String::from_utf8_lossy(&version).trim() == ABOUT_VERSION,
        "Expected {ABOUT_VERSION}"
    );
    let report: Report = serde_json::from_slice(&output(
        "cargo",
        &[
            "about",
            "generate",
            "--locked",
            "--offline",
            "--workspace",
            "--all-features",
            "--fail",
            "--format",
            "json",
            "--config",
            "about.toml",
        ],
    )?)?;
    // Build in a temporary directory so a failed scan cannot leave a partial bundle.
    fs::create_dir_all(
        destination
            .parent()
            .context("Missing license destination parent")?,
    )?;
    let temporary = tempfile::tempdir_in(destination.parent().unwrap())?;
    render(&report, temporary.path())?;
    if destination.exists() {
        fs::remove_dir_all(destination)?;
    }
    fs::rename(temporary.keep(), destination)?;
    println!("License bundle: {}", destination.display());
    Ok(())
}

/// Reuse a bundle produced for these dependencies by the shared CI job.
pub(super) fn copy_bundle(source: &Path, destination: &Path) -> Result<()> {
    validate_destination(destination)?;
    anyhow::ensure!(
        fs::read_to_string(source.join(".irodoru-license-bundle"))? == "1\n",
        "Invalid license bundle marker"
    );
    for name in [
        "LICENSE",
        "THIRD-PARTY-NOTICES.txt",
        "THIRD-PARTY-SOURCES.txt",
    ] {
        anyhow::ensure!(source.join(name).is_file(), "Missing license file: {name}");
    }
    for name in ["sources", "model"] {
        anyhow::ensure!(
            fs::read_dir(source.join(name))?.next().is_some(),
            "Empty license directory: {name}"
        );
    }
    let parent = destination
        .parent()
        .context("Missing license destination parent")?;
    fs::create_dir_all(parent)?;
    anyhow::ensure!(
        !parent.canonicalize()?.starts_with(source.canonicalize()?),
        "License destination must be outside the source bundle"
    );
    let temporary = tempfile::tempdir_in(parent)?;
    copy_files(source, temporary.path())?;
    if destination.exists() {
        fs::remove_dir_all(destination)?;
    }
    fs::rename(temporary.keep(), destination)?;
    println!("Reused license bundle: {}", destination.display());
    Ok(())
}

fn copy_files(source: &Path, destination: &Path) -> Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let path = destination.join(entry.file_name());
        if kind.is_dir() {
            fs::create_dir(&path)?;
            copy_files(&entry.path(), &path)?;
        } else {
            anyhow::ensure!(
                kind.is_file(),
                "License bundle must contain only regular files"
            );
            anyhow::ensure!(
                entry.metadata()?.len() > 0,
                "Empty license file: {}",
                entry.path().display()
            );
            fs::copy(entry.path(), path)?;
        }
    }
    Ok(())
}

fn validate_destination(destination: &Path) -> Result<()> {
    if let Ok(metadata) = fs::symlink_metadata(destination) {
        anyhow::ensure!(
            metadata.is_dir() && !metadata.is_symlink(),
            "License destination must be a real directory"
        );
        anyhow::ensure!(
            fs::read_dir(destination)?.next().is_none()
                || fs::read_to_string(destination.join(".irodoru-license-bundle"))
                    .ok()
                    .as_deref()
                    == Some("1\n"),
            "Refusing to replace a directory that is not an irodoru license bundle: {}",
            destination.display()
        );
    }
    Ok(())
}

pub(super) fn cli(args: &[String]) -> Result<()> {
    let destination = option(args, "--destination")?
        .map(PathBuf::from)
        .unwrap_or_else(|| root().join("target/licenses"));
    generate(&destination)
}
