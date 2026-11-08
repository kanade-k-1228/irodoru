use super::*;
use serde_json::{Value, json};
fn platform() -> &'static str {
    if cfg!(windows) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(target_os = "macos") {
        "aarch64-apple-darwin"
    } else {
        "x86_64-unknown-linux-gnu"
    }
}
fn configuration(
    root: &Path,
    stage: &Path,
    target: &str,
    formats: &str,
    libraries: &[String],
) -> Result<Value> {
    let mut config: Value =
        serde_json::from_slice(&fs::read(root.join("pkgs/app/packager.json"))?)?;
    config["binariesDir"] = json!(stage);
    config["outDir"] = json!(root.join("target").join(target).join("release/bundle"));
    config["targetTriple"] = json!(target);
    config["formats"] = json!(formats.split(',').collect::<Vec<_>>());
    if target.contains("apple") {
        config["macos"]["frameworks"] =
            json!(libraries.iter().map(|s| stage.join(s)).collect::<Vec<_>>());
    } else {
        let mut files: serde_json::Map<String, Value> = libraries
            .iter()
            .map(|name| {
                (
                    stage.join(name).to_string_lossy().into_owned(),
                    json!(format!("usr/bin/{name}")),
                )
            })
            .collect();
        files.insert(
            root.join("pkgs/app/linux/irodoru.xml")
                .to_string_lossy()
                .into_owned(),
            json!("usr/share/mime/packages/irodoru.xml"),
        );
        config["deb"]["files"] = json!(files);
        config["appimage"] = json!({"files": files});
    }
    for icon in config["icons"].as_array_mut().context("Missing icons")? {
        *icon = json!(root.join(icon.as_str().context("Invalid icon")?));
    }
    config["licenseFile"] =
        json!(root.join(config["licenseFile"].as_str().context("Missing license")?));
    for resource in config["resources"]
        .as_array_mut()
        .context("Missing resources")?
    {
        resource["src"] = json!(
            root.join(
                resource["src"]
                    .as_str()
                    .context("Missing resource source")?
            )
        );
    }
    Ok(config)
}
pub fn cli(args: &[String]) -> Result<()> {
    let target = option(args, "--target")?.unwrap_or_else(|| platform().into());
    anyhow::ensure!(
        [
            "x86_64-pc-windows-msvc",
            "aarch64-apple-darwin",
            "x86_64-unknown-linux-gnu"
        ]
        .contains(&target.as_str()),
        "Unsupported desktop target: {target}"
    );
    let windows = target.contains("windows");
    let mac = target.contains("apple");
    let root = root();
    let mut build = Command::new("cargo");
    build.args([
        "build",
        "--locked",
        "--release",
        "-p",
        "irodoru",
        "--features",
        "desktop",
        "--target",
        &target,
    ]);
    if mac {
        build.env("MACOSX_DEPLOYMENT_TARGET", "15.0");
    }
    anyhow::ensure!(build.status()?.success(), "Desktop build failed");
    let release = root.join("target").join(&target).join("release");
    let stage = release.join("native");
    if stage.exists() {
        fs::remove_dir_all(&stage)?;
    }
    fs::create_dir_all(&stage)?;
    let binary_name = if windows { "irodoru.exe" } else { "irodoru" };
    let binary = stage.join(binary_name);
    fs::copy(release.join(binary_name), &binary)?;
    let mut libraries = Vec::new();
    for entry in fs::read_dir(&release)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if if windows {
            name.ends_with(".dll")
        } else if mac {
            name.ends_with(".dylib")
        } else {
            name.ends_with(".so") || name.contains(".so.")
        } {
            fs::copy(entry.path(), stage.join(&name))?;
            libraries.push(name);
        }
    }
    let executable = binary.to_str().context("Invalid executable path")?;
    if mac {
        run(
            "install_name_tool",
            &["-add_rpath", "@executable_path/../Frameworks", executable],
        )?;
        run(
            "install_name_tool",
            &["-add_rpath", "@executable_path", executable],
        )?;
        run("codesign", &["--force", "--sign", "-", executable])?;
    } else if !windows {
        run("patchelf", &["--set-rpath", "$ORIGIN", executable])?;
    }
    let no_bundle = args.iter().any(|a| a == "--no-bundle");
    if windows || no_bundle {
        if no_bundle {
            return Ok(());
        }
        let output = release.join("bundle/portable/irodoru_windows.zip");
        fs::create_dir_all(output.parent().unwrap())?;
        if output.exists() {
            fs::remove_file(&output)?;
        }
        anyhow::ensure!(
            Command::new("7z")
                .args(["a", "-tzip", "-mx=0"])
                .arg(output)
                .arg(".")
                .current_dir(stage)
                .status()?
                .success(),
            "ZIP creation failed"
        );
    } else {
        let formats = option(args, "--formats")?
            .unwrap_or_else(|| if mac { "dmg" } else { "appimage,deb" }.into());
        let config = configuration(&root, &stage, &target, &formats, &libraries)?;
        let path = release.join("packager.json");
        json_write(&path, &config)?;
        run(
            "cargo",
            &[
                "packager",
                "--release",
                "--config",
                path.to_str().context("Invalid config path")?,
            ],
        )?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_paths_are_absolute_and_library_mapping_is_source_to_destination() {
        let root = root();
        let stage = root.join("target/test/native");
        let config = configuration(
            &root,
            &stage,
            "x86_64-unknown-linux-gnu",
            "deb,appimage",
            &["libonnxruntime.so".into()],
        )
        .unwrap();
        assert_eq!(
            config["deb"]["files"][stage.join("libonnxruntime.so").to_str().unwrap()],
            "usr/bin/libonnxruntime.so"
        );
        assert!(config["resources"].as_array().unwrap().is_empty());
        for format in ["deb", "appimage"] {
            assert_eq!(
                config[format]["files"][root.join("pkgs/app/linux/irodoru.xml").to_str().unwrap()],
                "usr/share/mime/packages/irodoru.xml"
            );
        }
        assert_eq!(config["fileAssociations"][0]["extensions"], json!(["iro"]));
        assert_eq!(
            config["fileAssociations"][0]["mimeType"],
            "application/x-irodoru-project"
        );
        // cargo-packager deserializes camelCase; its macOS plist writer emits "Editor".
        assert_eq!(config["fileAssociations"][0]["role"], "editor");
        let config = configuration(
            &root,
            &stage,
            "aarch64-apple-darwin",
            "dmg",
            &["libonnxruntime.dylib".into()],
        )
        .unwrap();
        assert_eq!(config["macos"]["signingIdentity"], "-");
        assert_eq!(
            config["macos"]["frameworks"][0],
            json!(stage.join("libonnxruntime.dylib"))
        );
    }
}
