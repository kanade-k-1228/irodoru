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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_files_rejoin_to_original() {
        let dir = tempfile::tempdir().unwrap();
        let target = "x86_64-pc-windows-msvc";
        let source = dir
            .path()
            .join("target")
            .join(target)
            .join("release/bundle");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("app.zip"), b"1234567890").unwrap();
        let dest = dir.path().join("assets");
        let assets = prepare(dir.path(), "0.0.5-beta.1", target, &dest, 4).unwrap();
        assert_eq!(assets.len(), 4);
        let joined: Vec<u8> = (1..=3)
            .flat_map(|i| {
                fs::read(dest.join(format!("irodoru_0.0.5-beta.1_windows_x64.zip.part{i:03}")))
                    .unwrap()
            })
            .collect();
        assert_eq!(joined, b"1234567890");
        let instructions =
            fs::read_to_string(dest.join("irodoru_0.0.5-beta.1_windows_x64_README.txt")).unwrap();
        assert!(instructions.contains("irodoru_0.0.5-beta.1_windows_x64.zip.part003"));
        fs::create_dir(source.join("nested")).unwrap();
        fs::write(source.join("nested/app.zip"), b"duplicate").unwrap();
        assert!(prepare(dir.path(), "0.0.5-beta.1", target, &dest, 4).is_err());
    }
    #[test]
    fn release_names_are_consistent_and_ignore_packager_intermediates() {
        for (target, source, expected) in [
            (
                "x86_64-pc-windows-msvc",
                "irodoru_windows.zip",
                "irodoru_0.0.4_windows_x64.zip",
            ),
            (
                "aarch64-apple-darwin",
                "irodoru_0.0.4_aarch64.dmg",
                "irodoru_0.0.4_macos_arm64.dmg",
            ),
            (
                "x86_64-unknown-linux-gnu",
                "irodoru_0.0.4_amd64.deb",
                "irodoru_0.0.4_linux_x64.deb",
            ),
            (
                "x86_64-unknown-linux-gnu",
                "irodoru_0.0.4_x86_64.AppImage",
                "irodoru_0.0.4_linux_x64.AppImage",
            ),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let bundle = dir
                .path()
                .join("target")
                .join(target)
                .join("release/bundle");
            let intermediate = bundle.join(".cargo-packager/appimage/app.AppDir");
            fs::create_dir_all(&intermediate).unwrap();
            fs::write(intermediate.join("temporary.AppImage"), b"not a release").unwrap();
            #[cfg(unix)]
            std::os::unix::fs::symlink("missing", intermediate.join("AppRun")).unwrap();
            fs::write(bundle.join(source), b"installer").unwrap();
            let dest = dir.path().join("assets");
            let assets = prepare(dir.path(), "0.0.4", target, &dest, 1024).unwrap();
            assert_eq!(assets, vec![dest.join(expected)]);
            assert_eq!(fs::read(&assets[0]).unwrap(), b"installer");
            assert_eq!(fs::read_dir(&dest).unwrap().count(), 1);
            assert!(prepare(dir.path(), "../bad", target, &dest, 1024).is_err());
            assert!(prepare(dir.path(), "0.0.4", "unsupported", &dest, 1024).is_err());
        }
    }
    #[test]
    fn version_updates_only_app_in_lock() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("pkgs/app")).unwrap();
        fs::write(
            dir.path().join("pkgs/app/Cargo.toml"),
            "[package]\nname = \"irodoru\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("pkgs/app/packager.json"),
            "{\"version\":\"0.1.0\"}",
        )
        .unwrap();
        fs::write(dir.path().join("Cargo.lock"),"version = 4\n[[package]]\nname = \"irodoru\"\nversion = \"0.1.0\"\n[[package]]\nname = \"other\"\nversion = \"0.1.0\"\n").unwrap();
        set_version(dir.path(), "1.2.3").unwrap();
        let config: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.path().join("pkgs/app/packager.json")).unwrap())
                .unwrap();
        assert_eq!(config["version"], "1.2.3");
        set_version(dir.path(), "1.2.3-beta.1").unwrap();
        let lock = fs::read_to_string(dir.path().join("Cargo.lock")).unwrap();
        assert!(lock.contains("version = \"1.2.3-beta.1\""));
        assert!(lock.contains("name = \"other\"\nversion = \"0.1.0\""));
        assert!(set_version(dir.path(), "oops").is_err());
    }
}
