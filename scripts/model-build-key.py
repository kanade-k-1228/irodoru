#!/usr/bin/env python3
"""Hash the locked dependencies of xtask without unrelated app dependencies."""
import hashlib
import json
from pathlib import Path
import sys
import tomllib


def dependency_key(lockfile: Path) -> str:
    packages = tomllib.loads(lockfile.read_text(encoding="utf-8"))["package"]

    def resolve(reference: str) -> int:
        parts = reference.split(" ", 2)
        matches = [
            index for index, package in enumerate(packages)
            if package["name"] == parts[0]
            and (len(parts) < 2 or package["version"] == parts[1])
            and (len(parts) < 3 or package.get("source") == parts[2].strip("()"))
        ]
        if len(matches) != 1:
            raise ValueError(f"Ambiguous or missing locked dependency: {reference}")
        return matches[0]

    pending = [resolve("xtask")]
    selected = set()
    while pending:
        index = pending.pop()
        if index in selected:
            continue
        selected.add(index)
        pending.extend(resolve(dep) for dep in packages[index].get("dependencies", []))
    def identity(package: dict) -> tuple[str, str, str]:
        return package["name"], package["version"], package.get("source", "")

    inputs = []
    for index in sorted(selected, key=lambda index: identity(packages[index])):
        package = dict(packages[index])
        # Cargo may add/remove version qualifiers when an unrelated duplicate goes away.
        package["dependencies"] = sorted(
            identity(packages[resolve(dep)]) for dep in package.get("dependencies", [])
        )
        inputs.append(package)
    return hashlib.sha256(json.dumps(inputs, sort_keys=True).encode()).hexdigest()


if __name__ == "__main__":
    print(dependency_key(Path(sys.argv[1] if len(sys.argv) > 1 else "Cargo.lock")))
