"""Create and publish the desktop model bundle without Node or third-party Python packages."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile

from model_archive import MAX_ARCHIVE_SIZE, create_archive


REQUIRED = {
    "anime": (
        "manifest.json", "tokenizer.json", "caption_tokenizer.json", "forced_splits.json",
        "backbone.onnx", "text_proj.onnx", "caption_proj.onnx", "speaker_encoder.onnx",
        "duration.onnx", "dit_context.onnx", "dit_step.onnx",
    ),
    "codec": ("codec.json", "dacvae_encoder.onnx", "dacvae_decoder.onnx"),
}
SHA256 = re.compile(r"[a-f0-9]{64}\Z")
MODEL_PATH = re.compile(r"(anime|codec)/[A-Za-z0-9_-](?:[A-Za-z0-9_.-]*[A-Za-z0-9_-])?\Z")


def fingerprint(path):
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            digest.update(chunk)
            size += len(chunk)
    return {"size": size, "sha256": digest.hexdigest()}


def valid_repository(repository):
    return isinstance(repository, str) and len(repository) <= 255 and bool(re.fullmatch(
        r"[a-z0-9](?:[a-z0-9._-]*[a-z0-9])?/[a-z0-9](?:[a-z0-9._-]*[a-z0-9])?", repository
    ))


def validate_files(files):
    if not isinstance(files, list) or not files:
        raise ValueError("Missing model files")
    paths = set()
    for entry in files:
        name = entry.get("path", "")
        if not MODEL_PATH.fullmatch(name):
            raise ValueError("Invalid distribution model path")
        if name in paths:
            raise ValueError("Duplicate model resource")
        paths.add(name)
        if type(entry.get("size")) is not int or not 0 < entry["size"] <= MAX_ARCHIVE_SIZE:
            raise ValueError("Empty or invalid model file")
        if not SHA256.fullmatch(entry.get("sha256", "")):
            raise ValueError("Invalid model file checksum")
    if any(f"{folder}/{name}" not in paths for folder, names in REQUIRED.items() for name in names):
        raise ValueError("Incomplete model resources")
    return paths


def distribute(source, destination, repository=None):
    if repository is not None and not valid_repository(repository):
        raise ValueError("Invalid GHCR model repository; expected lowercase owner/package")
    source, destination = source.resolve(strict=True), destination.resolve()
    if source == destination or source in destination.parents or destination in source.parents:
        raise ValueError("Model distribution directories must not overlap")
    index_bytes = (source / "bundle.json").read_bytes()
    index = json.loads(index_bytes)
    if index.get("model") != "anime":
        raise ValueError("App model resources must use anime")
    paths = validate_files(index["files"])
    actual = set()
    for path in source.rglob("*"):
        if path.is_symlink():
            raise ValueError("Symlink in model resources")
        if path.is_file():
            actual.add(path.relative_to(source).as_posix())
    if actual != paths | {"bundle.json"}:
        raise ValueError("Unexpected model resources; run cargo xtask models prepare")
    revision = hashlib.sha256(index_bytes).hexdigest()
    asset = f"{revision}-models.zip"
    destination.mkdir(parents=True, exist_ok=True)
    temporary = destination / f"{asset}.tmp"
    try:
        create_archive(source, temporary)
        archive = {"format": "zip", "asset": asset, **fingerprint(temporary)}
        temporary.replace(destination / asset)
    finally:
        temporary.unlink(missing_ok=True)
    manifest = {
        "schema": 2, "model": "anime", "revision": revision, "repository": repository,
        "archive": archive, "files": index["files"],
    }
    temporary_manifest = destination / "model-distribution.json.tmp"
    temporary_manifest.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    temporary_manifest.replace(destination / "model-distribution.json")
    for path in destination.iterdir():
        if path.is_file() and path.name not in {asset, "model-distribution.json"}:
            path.unlink()
    return manifest


def run_oras(args, directory):
    return subprocess.run(["oras", *args], cwd=directory, check=True, stdout=subprocess.PIPE).stdout


def publish(directory, run=run_oras):
    directory = directory.resolve(strict=True)
    distribution = json.loads((directory / "model-distribution.json").read_bytes())
    repository, revision = distribution.get("repository"), distribution.get("revision")
    if (distribution.get("schema") != 2 or distribution.get("model") != "anime"
            or not valid_repository(repository) or not isinstance(revision, str)
            or not SHA256.fullmatch(revision)):
        raise ValueError("Model distribution is not ready for GHCR publication")
    validate_files(distribution.get("files"))
    archive = distribution.get("archive") or {}
    if archive.get("format") != "zip" or archive.get("asset") != f"{revision}-models.zip":
        raise ValueError("Invalid model archive")
    if (type(archive.get("size")) is not int or not 0 < archive["size"] <= MAX_ARCHIVE_SIZE
            or not SHA256.fullmatch(archive.get("sha256", ""))):
        raise ValueError("Invalid model archive size or digest (GHCR layer limit: 10 GB)")
    if fingerprint(directory / archive["asset"]) != {key: archive[key] for key in ("size", "sha256")}:
        raise ValueError("Model archive checksum mismatch")
    with tempfile.TemporaryDirectory(prefix="irodoru-ghcr-") as temporary:
        temporary = Path(temporary)
        exported = temporary / "manifest.json"
        reference = f"ghcr.io/{repository}"
        run([
            "push", f"{reference}:{archive['sha256']}",
            "--artifact-type", "application/vnd.irodoru.models.v2",
            "--annotation", "org.opencontainers.image.created=1970-01-01T00:00:00Z",
            "--annotation", f"org.opencontainers.image.source=https://github.com/{repository.removesuffix('-models')}",
            "--annotation", f"org.opencontainers.image.revision={revision}",
            "--export-manifest", str(exported), f"{archive['asset']}:application/zip",
        ], directory)
        manifest_bytes = exported.read_bytes()
        manifest = json.loads(manifest_bytes)
        layers = manifest.get("layers", [])
        if (len(layers) != 1 or layers[0].get("digest") != f"sha256:{archive['sha256']}"
                or layers[0].get("size") != archive["size"] or layers[0].get("mediaType") != "application/zip"
                or layers[0].get("annotations", {}).get("org.opencontainers.image.title") != archive["asset"]):
            raise ValueError("Published OCI layer does not match the app model archive")
        digest = f"sha256:{hashlib.sha256(manifest_bytes).hexdigest()}"
        anonymous = temporary / "anonymous.json"
        anonymous.write_text('{"auths":{}}', encoding="utf-8")
        fetched = temporary / "public-manifest.json"
        try:
            run(["manifest", "fetch", f"{reference}@{digest}", "--registry-config", str(anonymous),
                 "--output", str(fetched)], directory)
            # HEAD checks public access without downloading several GB again.
            descriptor = json.loads(run([
                "blob", "fetch", f"{reference}@sha256:{archive['sha256']}", "--descriptor",
                "--registry-config", str(anonymous),
            ], directory))
        except (subprocess.CalledProcessError, OSError) as error:
            raise ValueError(
                f"Cannot download {reference} anonymously. New GHCR packages are private by default. "
                "Set the package visibility to Public in GitHub Package settings, then rerun this workflow."
            ) from error
        if fetched.read_bytes() != manifest_bytes:
            raise ValueError("Public OCI manifest digest mismatch")
        if descriptor.get("digest") != layers[0]["digest"] or descriptor.get("size") != archive["size"]:
            raise ValueError("Public OCI blob descriptor mismatch")
        print(f"Public model artifact verified: {reference}@{digest}")
        return digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("distribute", "publish"))
    parser.add_argument("--source", type=Path, default=Path("pkgs/app/resources/models"))
    parser.add_argument("--destination", type=Path, default=Path("model-assets"))
    parser.add_argument("--repository")
    args = parser.parse_args()
    if args.command == "distribute":
        distribute(args.source, args.destination, args.repository)
    else:
        publish(args.destination)


if __name__ == "__main__":
    main()
