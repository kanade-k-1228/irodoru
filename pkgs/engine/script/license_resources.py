"""Reviewed model inputs and notices, shared by local and container exports."""
import json
from pathlib import Path

CODEC_REPO = "Aratako/Semantic-DACVAE-Japanese-32dim"
IRODORI_REVISION = "7ed4374eaf49355279db51307e8141984f900836"
DACVAE_REVISION = "414c20785fc3a28373073ea8ef7a1316eeeaca6e"


def license_directory() -> Path:
    for parent in Path(__file__).resolve().parents:
        candidate = parent / "licenses" / "model"
        if (candidate / "revisions.json").is_file():
            return candidate
    raise FileNotFoundError("Missing reviewed model licenses; include licenses/model in the export environment")


def revisions() -> dict[str, str]:
    return json.loads((license_directory() / "revisions.json").read_text(encoding="utf-8"))


def write_notices(destination: Path, repository: str) -> None:
    revision = revisions()[repository]  # Unreviewed inputs must not be silently distributed.
    directory = license_directory()
    names = ["NOTICE.txt", "ETHICAL-RESTRICTIONS.txt", "irodori-LICENSE.txt",
             "sbintuitions-LICENSE.txt", "sarashina-LICENSE.txt", "dacvae-LICENSE.txt", "descript-LICENSE.txt"]
    text = "\n\n".join(f"--- {name} ---\n{(directory / name).read_text(encoding='utf-8')}" for name in names)
    destination.mkdir(parents=True, exist_ok=True)
    (destination / "LICENSES.txt").write_text(text + "\n", encoding="utf-8")
    (destination / "provenance.json").write_text(json.dumps({
        "repository": repository,
        "revision": revision,
        "irodori_tts_revision": IRODORI_REVISION,
        "dacvae_revision": DACVAE_REVISION,
        "modifications": "ONNX conversion; see LICENSES.txt",
    }, indent=2) + "\n", encoding="utf-8")
