use anyhow::{Context, Result, bail};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
mod models;
mod package;
mod release;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}
fn run(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("Run {program}"))?;
    anyhow::ensure!(status.success(), "{program} failed: {status}");
    Ok(())
}
fn output(program: &str, args: &[&str]) -> Result<Vec<u8>> {
    let result = Command::new(program)
        .args(args)
        .stderr(Stdio::inherit())
        .output()?;
    anyhow::ensure!(
        result.status.success(),
        "{program} failed: {}",
        result.status
    );
    Ok(result.stdout)
}
fn option(args: &[String], name: &str) -> Result<Option<String>> {
    if let Some(i) = args.iter().position(|a| a == name) {
        return Ok(Some(
            args.get(i + 1)
                .filter(|v| !v.starts_with("--"))
                .context(format!("Missing value for {name}"))?
                .clone(),
        ));
    }
    Ok(None)
}
fn json_write(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    fs::write(path, format!("{}\n", serde_json::to_string_pretty(value)?))?;
    Ok(())
}
fn generate(check: bool) -> Result<()> {
    let bytes = output(
        "cargo",
        &[
            "run",
            "--locked",
            "--quiet",
            "-p",
            "irodori-server",
            "--",
            "openapi",
        ],
    )?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let text = format!("{}\n", serde_json::to_string_pretty(&value)?);
    let path = root().join("pkgs/server/openapi.json");
    if check {
        anyhow::ensure!(
            fs::read_to_string(path)? == text,
            "OpenAPI is stale; run cargo xtask generate"
        );
    } else {
        fs::write(path, text)?;
    }
    Ok(())
}
fn main() -> Result<()> {
    env::set_current_dir(root())?;
    let args: Vec<String> = env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or_default();
    match args.first().map(String::as_str) {
        Some("dev") => {
            run(
                "cargo",
                &[
                    "run",
                    "--locked",
                    "-p",
                    "irodoru-desktop",
                    "--features",
                    "desktop",
                ],
            )?;
        }
        Some("models") => models::cli(rest)?,
        Some("package") => package::cli(rest)?,
        Some("release") => release::cli(rest)?,
        Some("generate") => {
            anyhow::ensure!(
                rest.iter().all(|s| s == "api" || s == "--check"),
                "Usage: cargo xtask generate [api] [--check]"
            );
            generate(rest.iter().any(|s| s == "--check"))?;
        }
        Some("verify") => {
            run(
                if cfg!(windows) { "python" } else { "python3" },
                &[
                    "-m",
                    "unittest",
                    "discover",
                    "-s",
                    "scripts",
                    "-p",
                    "test_*.py",
                ],
            )?;
            run("cargo", &["fmt", "--all", "--", "--check"])?;
            run(
                "cargo",
                &[
                    "clippy",
                    "--locked",
                    "--workspace",
                    "--all-targets",
                    "--",
                    "-D",
                    "warnings",
                ],
            )?;
            run("cargo", &["test", "--locked", "--workspace"])?;
            generate(true)?;
        }
        Some("verify-export") => run(
            "uv",
            &[
                "run",
                "--project",
                "pkgs/engine/script",
                "python",
                "-m",
                "unittest",
                "discover",
                "-s",
                "pkgs/engine/script",
                "-p",
                "test_*.py",
            ],
        )?,
        None | Some("help" | "--help") => println!(
            "cargo xtask dev | models export/prepare/check/distribute/publish | generate [--check] | package [--target TRIPLE] [--formats LIST] [--no-bundle] | release version/app/container | verify | verify-export"
        ),
        _ => bail!("Unknown task; run cargo xtask help"),
    }
    Ok(())
}
