"""Exercise package extraction checks with a tiny portable ZIP and fake runtime."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
import zipfile


@unittest.skipUnless(os.name == "posix" and shutil.which("bash"), "requires a POSIX shell for the mock runtime")
class PackageSmokeTests(unittest.TestCase):
    def test_external_models_are_used_and_model_weights_in_packages_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            tools = root / "bin"
            tools.mkdir()
            extractor = tools / "7z"
            extractor.write_text(
                "#!/usr/bin/env python3\n"
                "import pathlib, sys, zipfile\n"
                "out = pathlib.Path(sys.argv[3][2:])\n"
                "with zipfile.ZipFile(sys.argv[2]) as archive: archive.extractall(out)\n"
                "(out / 'irodoru.exe').chmod(0o755)\n"
            )
            extractor.chmod(0o755)
            models = root / "external-models"
            models.mkdir()
            (models / "bundle.json").write_text("{}")
            bundle = root / "target/x86_64-pc-windows-msvc/release/bundle"
            bundle.mkdir(parents=True)
            runtime = '#!/usr/bin/env bash\nset -e\ntest "$1" = --smoke-test\ntest -f "$IRODORU_MODELS/bundle.json"\ntouch "$SMOKE_MARKER"\n'
            archive = bundle / "app.zip"
            marker = root / "runtime-ran"
            environment = {**os.environ, "PATH": f"{tools}{os.pathsep}{os.environ['PATH']}",
                           "IRODORU_TEST_MODELS": str(models), "SMOKE_MARKER": str(marker)}
            script = Path(__file__).with_name("smoke-package.sh").resolve()
            for weights, licenses in ((False, True), (True, True), (False, False)):
                with zipfile.ZipFile(archive, "w") as output:
                    output.writestr("irodoru.exe", runtime)
                    if licenses:
                        for name in ("LICENSE", "THIRD-PARTY-NOTICES.txt", "THIRD-PARTY-SOURCES.txt",
                                     "model/ETHICAL-RESTRICTIONS.txt", "sources/symphonia-0.6.1.crate",
                                     "sources/option-ext-0.2.0.crate"):
                            output.writestr("licenses/" + name, "notice or source fixture")
                    if weights:
                        output.writestr("models/anime/backbone.onnx.data", "accidental weights")
                result = subprocess.run(["bash", str(script), "x86_64-pc-windows-msvc"], cwd=root,
                                        env=environment, capture_output=True, text=True)
                if weights:
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("contains model weights", result.stderr)
                    self.assertFalse(marker.exists(), "Do not run packages that accidentally contain models")
                elif not licenses:
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("missing third-party licenses", result.stderr)
                    self.assertFalse(marker.exists())
                else:
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertTrue(marker.exists(), "Extracted runtime must see the external model fixture")
                    marker.unlink()


if __name__ == "__main__":
    unittest.main()
