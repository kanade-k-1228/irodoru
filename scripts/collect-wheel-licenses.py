"""Preserve native OpenVINO wheel notices when only its libs/ are shipped."""
import shutil
import sys
from pathlib import Path


def collect(source: Path, destination: Path) -> None:
    found = []
    for path in sorted(source.rglob("*")):
        relative = path.relative_to(source)
        # Keep license directories and third-party texts inside the package as
        # well as dist-info/licenses. METADATA identifies the installed version.
        if path.is_file() and (path.name == "METADATA" or any(
            "license" in part.lower() or "notice" in part.lower() or part.upper().startswith("COPYING")
            for part in relative.parts
        )):
            target = destination / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, target)
            found.append(relative)
    if not any("openvino" in str(path).lower() and "license" in str(path).lower() for path in found):
        raise RuntimeError("OpenVINO wheel has no license texts; review the wheel layout")


if __name__ == "__main__":
    collect(Path(sys.argv[1]), Path(sys.argv[2]))
