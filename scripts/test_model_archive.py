import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

from model_archive import create_archive


class ArchiveTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.source = self.root / "source"
        self.source.mkdir()
        self.content = {"anime/model.onnx": b"model" * 300_000, "codec/codec.json": b"{}"}
        self.files = []
        for name, content in self.content.items():
            path = self.source / name
            path.parent.mkdir(exist_ok=True)
            path.write_bytes(content)
            self.files.append({"path": name, "size": len(content), "sha256": hashlib.sha256(content).hexdigest()})
        self.write_index(self.files)

    def write_index(self, files):
        (self.source / "bundle.json").write_text(json.dumps({"files": files}))

    def test_round_trip_and_deterministic_metadata(self):
        first = self.root / "first.zip"
        second = self.root / "second.zip"
        create_archive(self.source, first)
        for name in self.content:
            os.utime(self.source / name, (1_800_000_000, 1_800_000_000))
            (self.source / name).chmod(0o600)
        self.write_index(list(reversed(self.files)))
        create_archive(self.source, second)
        self.assertEqual(first.read_bytes(), second.read_bytes())
        with zipfile.ZipFile(first) as archive:
            self.assertEqual(archive.namelist(), sorted(self.content))
            for info in archive.infolist():
                self.assertEqual(archive.read(info), self.content[info.filename])
                self.assertEqual(info.compress_type, zipfile.ZIP_STORED)
                self.assertEqual(info.date_time, (1980, 1, 1, 0, 0, 0))
                self.assertEqual(info.create_system, 3)
                self.assertEqual(info.external_attr >> 16, 0o100644)
                self.assertEqual(info.extract_version, 45)
                self.assertFalse(info.is_dir())

    def test_zip64_directory_without_a_multi_gigabyte_fixture(self):
        output = self.root / "zip64.zip"
        with patch.object(zipfile, "ZIP64_LIMIT", 100):
            create_archive(self.source, output)
        self.assertIn(b"PK\x06\x06", output.read_bytes())
        with zipfile.ZipFile(output) as archive:
            for name, content in self.content.items():
                self.assertEqual(archive.read(name), content)

    def test_rejects_changed_content_unsafe_paths_and_layer_limit(self):
        output = self.root / "invalid.zip"
        for files in [[self.files[0], self.files[0]], [{**self.files[0], "path": "../outside"}]]:
            self.write_index(files)
            with self.assertRaises(ValueError):
                create_archive(self.source, output)
        self.write_index(self.files)
        with patch("model_archive.MAX_ARCHIVE_SIZE", 1):
            with self.assertRaisesRegex(ValueError, "10 GB"):
                create_archive(self.source, output)
        # Account for ZIP headers too, not only the sum of model file sizes.
        with patch("model_archive.MAX_ARCHIVE_SIZE", sum(entry["size"] for entry in self.files)):
            with self.assertRaisesRegex(ValueError, "10 GB"):
                create_archive(self.source, output)
        (self.source / self.files[0]["path"]).write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "changed while archiving"):
            create_archive(self.source, output)


if __name__ == "__main__":
    unittest.main()
