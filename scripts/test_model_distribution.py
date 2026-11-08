import copy
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import zipfile

from model_distribution import MAX_ARCHIVE_SIZE, REQUIRED, distribute, fingerprint, publish, valid_repository


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

    def test_rejects_overlap_bad_repository_and_bundle(self):
        for destination in (self.source, self.source / "nested", self.root):
            with self.assertRaisesRegex(ValueError, "overlap"):
                distribute(self.source, destination)
        for repository in ("", "Example/app", "ghcr.io/a/b", "a/b:tag", "a/-b", "a/b.", "x/" + "a" * 254):
            self.assertFalse(valid_repository(repository))
            with self.assertRaisesRegex(ValueError, "Invalid GHCR"):
                distribute(self.source, self.output, repository)
        for patch in ({"path": "../escape"}, {"size": 0}, {"sha256": "invalid"}):
            index = copy.deepcopy(self.index)
            index["files"][0].update(patch)
            self.write_index(index)
            with self.assertRaises(ValueError):
                self.manifest()
        for files in (self.index["files"][1:], self.index["files"] + [self.index["files"][0]]):
            self.write_index({**self.index, "files": files})
            with self.assertRaises(ValueError):
                self.manifest()
        self.write_index(self.index)
        (self.source / "unexpected").write_text("stale")
        with self.assertRaisesRegex(ValueError, "Unexpected"):
            self.manifest()
        (self.source / "unexpected").unlink()
        (self.source / self.index["files"][0]["path"]).write_text("corrupted")
        with self.assertRaisesRegex(ValueError, "changed while archiving"):
            self.manifest()
        self.assertFalse(list(self.output.glob("*.tmp")))

    def mock_oras(self, distribution, *, denied=None, layers=None, wrong_public=False, wrong_blob=False):
        archive = distribution["archive"]
        layer = {"digest": f"sha256:{archive['sha256']}", "size": archive["size"], "mediaType": "application/zip",
                 "annotations": {"org.opencontainers.image.title": archive["asset"]}}
        payload = json.dumps({"schemaVersion": 2, "layers": [layer] if layers is None else layers}).encode()
        digest = f"sha256:{hashlib.sha256(payload).hexdigest()}"
        calls = []

        def run(args, cwd):
            self.assertEqual(cwd, self.output)
            calls.append(args)
            if args[0] == denied:
                raise subprocess.CalledProcessError(1, "oras")
            if args[0] == "push":
                self.assertEqual(args[1], f"ghcr.io/example/app-models:{archive['sha256']}")
                self.assertEqual([arg for arg in args if arg.endswith(":application/zip")], [archive["asset"] + ":application/zip"])
                self.assertIn("org.opencontainers.image.created=1970-01-01T00:00:00Z", args)
                self.assertIn("org.opencontainers.image.source=https://github.com/example/app", args)
                self.assertIn(f"org.opencontainers.image.revision={distribution['revision']}", args)
                Path(args[args.index("--export-manifest") + 1]).write_bytes(payload)
            else:
                self.assertEqual(json.loads(Path(args[args.index("--registry-config") + 1]).read_bytes()), {"auths": {}})
                if args[0] == "manifest":
                    self.assertEqual(args[2], f"ghcr.io/example/app-models@{digest}")
                    Path(args[args.index("--output") + 1]).write_bytes(b"wrong" if wrong_public else payload)
                else:
                    self.assertEqual(args[:3], ["blob", "fetch", f"ghcr.io/example/app-models@sha256:{archive['sha256']}"])
                    self.assertIn("--descriptor", args)
                    return json.dumps({**layer, "size": -1} if wrong_blob else layer).encode()
            return b""

        return run, digest, calls

    def test_publish_one_layer_and_verify_anonymous_manifest_and_blob(self):
        distribution = self.manifest()
        run, digest, calls = self.mock_oras(distribution)
        self.assertEqual(publish(self.output, run), digest)
        self.assertEqual([args[0] for args in calls], ["push", "manifest", "blob"])
        self.assertEqual(publish(self.output, run), digest)
        for denied in ("manifest", "blob"):
            run, _, _ = self.mock_oras(distribution, denied=denied)
            with self.assertRaisesRegex(ValueError, "private by default.*Public.*rerun"):
                publish(self.output, run)
        for options, error in (({"wrong_public": True}, "manifest digest"), ({"wrong_blob": True}, "blob descriptor"),
                               ({"layers": []}, "Published OCI layer"), ({"layers": [{}, {}]}, "Published OCI layer")):
            run, _, _ = self.mock_oras(distribution, **options)
            with self.assertRaisesRegex(ValueError, error):
                publish(self.output, run)

    def test_publish_rejects_invalid_metadata_and_corrupt_archive_before_oras(self):
        distribution = self.manifest()
        def no_run(*_):
            self.fail("Invalid archives must never be published")
        for patch in ({"size": 0}, {"size": MAX_ARCHIVE_SIZE + 1}, {"size": True},
                      {"asset": "../escape.zip"}, {"format": "tar"}, {"sha256": "invalid"}):
            changed = {**distribution, "archive": {**distribution["archive"], **patch}}
            (self.output / "model-distribution.json").write_text(json.dumps(changed))
            with self.assertRaisesRegex(ValueError, "Invalid model archive"):
                publish(self.output, no_run)
        (self.output / "model-distribution.json").write_text(json.dumps(distribution))
        (self.output / distribution["archive"]["asset"]).write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            publish(self.output, no_run)
        distribute(self.source, self.output)
        with self.assertRaisesRegex(ValueError, "not ready"):
            publish(self.output, no_run)

    def test_cli_creates_the_same_artifacts(self):
        expected = self.manifest()
        result = subprocess.run([sys.executable, str(Path(__file__).with_name("model_distribution.py")),
                                 "distribute", "--source", str(self.source), "--destination", str(self.output),
                                 "--repository", "example/app-models"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads((self.output / "model-distribution.json").read_bytes()), expected)


if __name__ == "__main__":
    unittest.main()
