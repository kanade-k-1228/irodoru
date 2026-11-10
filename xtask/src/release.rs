use crate::{json_write, root};
use anyhow::{Context, Result, bail};
use std::{
    collections::HashSet,
    io::{Read, Write},
};
use std::{
    fs,
    path::{Path, PathBuf},
};
fn set_version(root: &Path, tag: &str) -> Result<()> {
    let version = tag;
    semver::Version::parse(version).context("Release tag must be MAJOR.MINOR.PATCH")?;
    let manifest = root.join("pkgs/app/Cargo.toml");
    let lock = root.join("Cargo.lock");
    let config_path = root.join("pkgs/app/packager.json");
    let mut app: toml_edit::DocumentMut = fs::read_to_string(&manifest)?.parse()?;
    let mut locked: toml_edit::DocumentMut = fs::read_to_string(&lock)?.parse()?;
    let package = locked["package"]
        .as_array_of_tables_mut()
        .context("Missing lock packages")?
        .iter_mut()
        .find(|p| p["name"].as_str() == Some("irodoru"))
        .context("Missing app lock entry")?;
    package["version"] = toml_edit::value(version);
    app["package"]["version"] = toml_edit::value(version);
    let mut config: serde_json::Value = serde_json::from_slice(&fs::read(&config_path)?)?;
    config["version"] = serde_json::json!(version);
    fs::write(manifest, app.to_string())?;
    fs::write(lock, locked.to_string())?;
    json_write(&config_path, &config)
}
// Packager retains intermediate AppDirs (including symlinks) under .cargo-packager.
// Only final, regular installer files belong in a release.
fn installer_files(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let kind = entry.file_type()?;
        if kind.is_dir() {
            files.extend(installer_files(&entry.path())?);
        } else if kind.is_file() {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}
fn prepare(
    root: &Path,
    tag: &str,
    target: &str,
    destination: &Path,
    part_size: u64,
) -> Result<Vec<PathBuf>> {
    let version = semver::Version::parse(tag).context("Release tag must be MAJOR.MINOR.PATCH")?;
    let platform = match target {
        "x86_64-pc-windows-msvc" => "windows_x64",
        "aarch64-apple-darwin" => "macos_arm64",
        "x86_64-unknown-linux-gnu" => "linux_x64",
        _ => bail!("Unsupported desktop target: {target}"),
    };
    let prefix = format!("irodoru_{version}_{platform}");
    anyhow::ensure!(
        part_size > 0 && part_size < 1 << 31,
        "Invalid release part size"
    );
    fs::create_dir_all(destination)?;
    let mut assets = Vec::new();
    let mut names = HashSet::new();
    let mut instructions = String::from(
        "Download all parts for your platform into one folder. Verify each part against its SHA-256 digest on the GitHub release page, then join them before opening or installing the app.\nWindows: use Command Prompt (cmd.exe), extract the ZIP and run irodoru.exe.\n",
    );
    let mut split = false;
    for file in installer_files(&root.join("target").join(target).join("release/bundle"))? {
        let ext = file
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        let ext = match ext.as_str() {
            "appimage" => "AppImage",
            "exe" | "dmg" | "deb" | "zip" => ext.as_str(),
            _ => continue,
        };
        let name = format!("{prefix}.{ext}");
        anyhow::ensure!(
            names.insert(name.clone()),
            "Duplicate installer name: {name}"
        );
        let size = fs::metadata(&file)?.len();
        let count = size.div_ceil(part_size).max(1);
        let mut input = fs::File::open(file)?;
        let mut parts = Vec::new();
        for part in 0..count {
            let filename = if count == 1 {
                name.clone()
            } else {
                format!("{name}.part{:03}", part + 1)
            };
            let path = destination.join(&filename);
            let mut out = fs::File::create(&path)?;
            let mut remaining = part_size;
            let mut buffer = [0u8; 65536];
            while remaining > 0 {
                let max = (remaining as usize).min(buffer.len());
                let n = input.read(&mut buffer[..max])?;
                if n == 0 {
                    break;
                }
                out.write_all(&buffer[..n])?;
                remaining -= n as u64;
            }
            assets.push(path);
            parts.push(filename);
        }
        if count > 1 {
            split = true;
            instructions.push_str(&format!(
                "\n{name}\nWindows: copy /b {} \"{name}\"\nmacOS/Linux: cat {} > '{name}'\n",
                parts
                    .iter()
                    .map(|s| format!("\"{s}\""))
                    .collect::<Vec<_>>()
                    .join("+"),
                parts
                    .iter()
                    .map(|s| format!("'{s}'"))
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
    }
    anyhow::ensure!(!names.is_empty(), "No installers found");
    if split {
        let path = destination.join(format!("{prefix}_README.txt"));
        fs::write(&path, instructions)?;
        assets.push(path);
    }
    assets.sort();
    Ok(assets)
}
pub fn cli(args: &[String]) -> Result<()> {
    if args.first().map(String::as_str) == Some("version") {
        anyhow::ensure!(args.len() == 2, "Usage: release version TAG");
        return set_version(&root(), &args[1]);
    }
    match args.first().map(String::as_str) {
        Some("app") if args.len() == 4 => {
            prepare(
                &root(),
                &args[1],
                &args[2],
                Path::new(&args[3]),
                1024 * 1024 * 1024,
            )?;
        }
        _ => bail!("Usage: cargo xtask release version TAG | app TAG TARGET DEST"),
    }
    Ok(())
}
