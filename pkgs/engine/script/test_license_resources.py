import json
from pathlib import Path
import tempfile
import unittest

from license_resources import CODEC_REPO, revisions, write_notices


class ModelLicenseTests(unittest.TestCase):
    def test_export_preserves_terms_and_records_reviewed_revision(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            for repo in (CODEC_REPO, "phasefield-audio/Irodori-TTS-v4.1-Anime"):
                write_notices(directory, repo)
                provenance = json.loads((directory / "provenance.json").read_text())
                self.assertEqual(provenance["repository"], repo)
                self.assertEqual(provenance["revision"], revisions()[repo])
                text = (directory / "LICENSES.txt").read_text()
                for required in ("Copyright (c) 2026 Aratako", "Copyright (c) 2025 SB Intuitions",
                                 "Copyright (c) 2024 SB Intuitions", "Apache License",
                                 "No Impersonation", "No Misinformation", "discussions/1"):
                    self.assertIn(required, text)

    def test_unreviewed_model_is_rejected_before_writing(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary) / "export"
            with self.assertRaises(KeyError):
                write_notices(directory, "unreviewed/model")
            self.assertFalse(directory.exists())


if __name__ == "__main__":
    unittest.main()
