use super::*;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    io::{Read, Write},
};
fn set_version(root: &Path, tag: &str) -> Result<()> {
    let version = tag.strip_prefix('v').unwrap_or(tag);
    semver::Version::parse(version).context("Release tag must be vMAJOR.MINOR.PATCH")?;
    let manifest = root.join("pkgs/app/Cargo.toml");
    let lock = root.join("Cargo.lock");
    let config_path = root.join("pkgs/app/packager.json");
    let mut app: toml_edit::DocumentMut = fs::read_to_string(&manifest)?.parse()?;
    let mut locked: toml_edit::DocumentMut = fs::read_to_string(&lock)?.parse()?;
    let package = locked["package"]
        .as_array_of_tables_mut()
        .context("Missing lock packages")?
        .iter_mut()
        .find(|p| p["name"].as_str() == Some("irodoru-desktop"))
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
fn prepare(root: &Path, target: &str, destination: &Path, part_size: u64) -> Result<Vec<PathBuf>> {
    anyhow::ensure!(
        !target.is_empty()
            && target
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_'),
        "Invalid target"
    );
    anyhow::ensure!(
        part_size > 0 && part_size < 1 << 31,
        "Invalid release part size"
    );
    fs::create_dir_all(destination)?;
    let mut assets = Vec::new();
    let mut names = HashSet::new();
    let mut instructions = String::from(
        "Download all parts for your platform into one folder. Join them before opening or installing the app.\nWindows: use Command Prompt (cmd.exe), extract the ZIP and run irodoru.exe.\nVerify the joined file: Get-FileHash FILE -Algorithm SHA256 (PowerShell), shasum -a 256 -c FILE.sha256 (macOS), sha256sum -c FILE.sha256 (Linux).\n",
    );
    let mut split = false;
    for file in installer_files(&root.join("target").join(target).join("release/bundle"))? {
        let ext = file.extension().unwrap_or_default().to_string_lossy();
        if !["exe", "dmg", "deb", "appimage", "zip"].contains(&ext.to_lowercase().as_str()) {
            continue;
        }
        let name = format!(
            "{}-{target}.{ext}",
            file.file_stem().unwrap().to_string_lossy()
        );
        anyhow::ensure!(
            names.insert(name.clone()),
            "Duplicate installer name: {name}"
        );
        let size = fs::metadata(&file)?.len();
        let count = size.div_ceil(part_size).max(1);
        let mut input = fs::File::open(file)?;
        let mut hash = Sha256::new();
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
                hash.update(&buffer[..n]);
                remaining -= n as u64;
            }
            assets.push(path);
            parts.push(filename);
        }
        let checksum = destination.join(format!("{name}.sha256"));
        let digest: String = hash
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        fs::write(&checksum, format!("{digest}  {name}\n"))?;
        assets.push(checksum);
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
        let path = destination.join(format!("irodoru-{target}-README.txt"));
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
    let temp = tempfile::tempdir()?;
    let (tag, assets) = match args.first().map(String::as_str) {
        Some("app") if args.len() == 3 => (
            args[1].clone(),
            prepare(&root(), &args[2], temp.path(), 1024 * 1024 * 1024)?,
        ),
        Some("container") if args.len() == 1 => {
            let tag = env::var("RELEASE_TAG")?;
            let image = env::var("IMAGE")?.to_lowercase();
            let version = env::var("IMAGE_TAG")?;
            let manifest: serde_json::Value = serde_json::from_slice(&output(
                "docker",
                &[
                    "buildx",
                    "imagetools",
                    "inspect",
                    &format!("{image}:{version}"),
                    "--format",
                    "{{json .Manifest}}",
                ],
            )?)?;
            let digest = manifest["digest"].as_str().context("Missing digest")?;
            anyhow::ensure!(
                digest.starts_with("sha256:")
                    && digest.len() == 71
                    && digest[7..].chars().all(|c| c.is_ascii_hexdigit()),
                "Invalid container digest"
            );
            let path = temp.path().join("container-image.txt");
            fs::write(
                &path,
                format!(
                    "irodoru synthesis server (linux/amd64, linux/arm64)\n\ndocker pull {image}:{version}\ndocker pull {image}@{digest}\n\ndocker run --rm -p 127.0.0.1:8080:8080 {image}:{version}\n"
                ),
            )?;
            (tag, vec![path])
        }
        _ => bail!("Usage: cargo xtask release version TAG | app TAG TARGET | container"),
    };
    anyhow::ensure!(
        Command::new("gh")
            .args(["release", "upload", &tag])
            .args(assets)
            .arg("--clobber")
            .status()?
            .success(),
        "Release upload failed"
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_files_rejoin_and_checksum_matches_original() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("target/test/release/bundle");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("app.zip"), b"1234567890").unwrap();
        let dest = dir.path().join("assets");
        let assets = prepare(dir.path(), "test", &dest, 4).unwrap();
        assert_eq!(assets.len(), 5);
        let joined: Vec<u8> = (1..=3)
            .flat_map(|i| fs::read(dest.join(format!("app-test.zip.part{i:03}"))).unwrap())
            .collect();
        assert_eq!(joined, b"1234567890");
        let expected = models::fingerprint(&source.join("app.zip")).unwrap().1;
        assert!(
            fs::read_to_string(dest.join("app-test.zip.sha256"))
                .unwrap()
                .starts_with(&expected)
        );
        fs::create_dir(source.join("nested")).unwrap();
        fs::write(source.join("nested/app.zip"), b"duplicate").unwrap();
        assert!(prepare(dir.path(), "test", &dest, 4).is_err());
    }
    #[test]
    fn release_ignores_packager_intermediates() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = dir.path().join("target/test/release/bundle");
        let intermediate = bundle.join(".cargo-packager/appimage/app.AppDir");
        fs::create_dir_all(&intermediate).unwrap();
        fs::write(intermediate.join("temporary.AppImage"), b"not a release").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("missing", intermediate.join("AppRun")).unwrap();
        fs::write(bundle.join("app.AppImage"), b"installer").unwrap();
        let assets = prepare(dir.path(), "test", &dir.path().join("assets"), 1024).unwrap();
        assert_eq!(assets.len(), 2);
        assert!(
            assets
                .iter()
                .all(|p| !p.to_string_lossy().contains("temporary"))
        );
    }
    #[test]
    fn version_updates_only_app_in_lock() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("pkgs/app")).unwrap();
        fs::write(
            dir.path().join("pkgs/app/Cargo.toml"),
            "[package]\nname = \"irodoru-desktop\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("pkgs/app/packager.json"),
            "{\"version\":\"0.1.0\"}",
        )
        .unwrap();
        fs::write(dir.path().join("Cargo.lock"),"version = 4\n[[package]]\nname = \"irodoru-desktop\"\nversion = \"0.1.0\"\n[[package]]\nname = \"other\"\nversion = \"0.1.0\"\n").unwrap();
        set_version(dir.path(), "v1.2.3-beta.1").unwrap();
        let lock = fs::read_to_string(dir.path().join("Cargo.lock")).unwrap();
        assert!(lock.contains("version = \"1.2.3-beta.1\""));
        assert!(lock.contains("name = \"other\"\nversion = \"0.1.0\""));
        assert!(set_version(dir.path(), "oops").is_err());
    }
}
