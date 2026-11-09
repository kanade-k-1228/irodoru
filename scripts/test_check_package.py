"""Exercise package content checks with a tiny portable ZIP."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
import zipfile


@unittest.skipUnless(os.name == "posix" and shutil.which("bash"), "requires a POSIX shell")
class PackageContentTests(unittest.TestCase):
    def test_packages_require_licenses_and_exclude_model_weights(self):
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
            )
            extractor.chmod(0o755)
            bundle = root / "target/x86_64-pc-windows-msvc/release/bundle"
            bundle.mkdir(parents=True)
            archive = bundle / "app.zip"
            environment = {**os.environ, "PATH": f"{tools}{os.pathsep}{os.environ['PATH']}"}
            script = Path(__file__).with_name("check-package.sh").resolve()
            for weights, licenses in ((False, True), (True, True), (False, False)):
                with zipfile.ZipFile(archive, "w") as output:
                    output.writestr("irodoru.exe", "placeholder executable")
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
                elif not licenses:
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("missing third-party licenses", result.stderr)
                else:
                    self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
