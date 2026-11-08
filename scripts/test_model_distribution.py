import hashlib
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

from model_distribution import REQUIRED, distribute, fingerprint, publish, valid_repository


class DistributionTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.source = self.root / "models"
        self.output = self.root / "assets"
        files = []
        for folder, names in REQUIRED.items():
            (self.source / folder).mkdir(parents=True)
            for name in names:
                path = self.source / folder / name
                path.write_bytes(b'{"name":"anime"}' if name == "manifest.json" else b"model fixture")
                files.append({"path": f"{folder}/{name}", **fingerprint(path)})
        path = self.source / "anime/backbone.onnx.data"
        path.write_bytes(bytes(range(256)))
        files.append({"path": "anime/backbone.onnx.data", **fingerprint(path)})
        self.index = {"model": "anime", "checkpoint": "fixture", "files": files}
        self.write_index(self.index)

    def write_index(self, index):
        (self.source / "bundle.json").write_text(json.dumps(index))

    def manifest(self):
        return distribute(self.source, self.output, "example/app-models")

    def test_deterministic_archive_and_content_revision(self):
        first = self.manifest()
        payload = (self.output / first["archive"]["asset"]).read_bytes()
        self.assertEqual(first["schema"], 2)
        self.assertEqual(first["revision"], hashlib.sha256((self.source / "bundle.json").read_bytes()).hexdigest())
        self.assertEqual(first["archive"]["sha256"], hashlib.sha256(payload).hexdigest())
        with zipfile.ZipFile(self.output / first["archive"]["asset"]) as archive:
            self.assertEqual(set(archive.namelist()), {entry["path"] for entry in self.index["files"]})
            self.assertTrue(all(info.compress_type == zipfile.ZIP_STORED for info in archive.infolist()))
        (self.output / "stale.part001").write_bytes(b"stale")
        self.assertEqual(self.manifest(), first)
        self.assertEqual((self.output / first["archive"]["asset"]).read_bytes(), payload)
        self.assertFalse((self.output / "stale.part001").exists())
        unpublished = distribute(self.source, self.output)
        self.assertIsNone(unpublished["repository"])
        self.assertEqual(unpublished["archive"], first["archive"])
        self.index["checkpoint"] = "new revision"
        self.write_index(self.index)
        changed = self.manifest()
        self.assertNotEqual(changed["revision"], first["revision"])
        self.assertFalse((self.output / first["archive"]["asset"]).exists())

    def test_rejects_invalid_destination_and_repository(self):
        with self.assertRaisesRegex(ValueError, "overlap"):
            distribute(self.source, self.source)
        self.assertFalse(valid_repository("../models"))
        with self.assertRaises(ValueError):
            distribute(self.source, self.output, "../models")

    def test_publishes_and_verifies_the_archive(self):
        distribution = self.manifest()
        archive = distribution["archive"]
        layer = {"digest": f"sha256:{archive['sha256']}", "size": archive["size"],
                 "mediaType": "application/zip",
                 "annotations": {"org.opencontainers.image.title": archive["asset"]}}
        payload = json.dumps({"schemaVersion": 2, "layers": [layer]}).encode()
        calls = []

        def run(args, cwd):
            calls.append(args[0])
            if args[0] == "push":
                Path(args[args.index("--export-manifest") + 1]).write_bytes(payload)
            elif args[0] == "manifest":
                Path(args[args.index("--output") + 1]).write_bytes(payload)
            else:
                return json.dumps(layer).encode()
            return b""

        self.assertEqual(publish(self.output, run), f"sha256:{hashlib.sha256(payload).hexdigest()}")
        self.assertEqual(calls, ["push", "manifest", "blob"])
        (self.output / archive["asset"]).write_bytes(b"corrupted")
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            publish(self.output, run)
        self.assertEqual(calls, ["push", "manifest", "blob"])


if __name__ == "__main__":
    unittest.main()
