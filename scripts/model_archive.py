"""Build a deterministic, uncompressed ZIP64 archive from a checked model bundle."""

import hashlib
import json
from pathlib import Path
import re
import sys
import zipfile


MAX_ARCHIVE_SIZE = 10_000_000_000


def create_archive(source: Path, output: Path) -> None:
    files = json.loads((source / "bundle.json").read_text())["files"]
    names = set()
    for entry in files:
        name = entry["path"]
        if not re.fullmatch(r"(anime|codec)/[A-Za-z0-9_-](?:[A-Za-z0-9_.-]*[A-Za-z0-9_-])?", name):
            raise ValueError("Invalid archive model path")
        if name in names:
            raise ValueError("Duplicate archive model path")
        names.add(name)
    if sum(entry["size"] for entry in files) > MAX_ARCHIVE_SIZE:
        raise ValueError("Model archive exceeds GHCR's 10 GB layer limit")
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_STORED, allowZip64=True) as archive:
        for entry in sorted(files, key=lambda entry: entry["path"]):
            info = zipfile.ZipInfo(entry["path"], date_time=(1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = 0o100644 << 16
            info.compress_type = zipfile.ZIP_STORED
            info.file_size = entry["size"]
            digest = hashlib.sha256()
            size = 0
            with (source / entry["path"]).open("rb") as original:
                with archive.open(info, "w", force_zip64=True) as target:
                    while chunk := original.read(1024 * 1024):
                        target.write(chunk)
                        digest.update(chunk)
                        size += len(chunk)
            if size != entry["size"] or digest.hexdigest() != entry["sha256"]:
                raise ValueError(f"Model changed while archiving: {entry['path']}")
    if output.stat().st_size > MAX_ARCHIVE_SIZE:
        raise ValueError("Model archive exceeds GHCR's 10 GB layer limit")


if __name__ == "__main__":
    create_archive(Path(sys.argv[1]), Path(sys.argv[2]))
