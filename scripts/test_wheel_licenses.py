import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("wheel_licenses", Path(__file__).with_name("collect-wheel-licenses.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class WheelLicenseTests(unittest.TestCase):
    def test_keeps_distribution_and_embedded_native_notices(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, output = root / "wheel", root / "notices"
            names = ("openvino-1.dist-info/licenses/LICENSE", "openvino/libs/third-party-notices.txt",
                     "openvino-1.dist-info/METADATA")
            for name in (*names, "openvino/libs/libopenvino.so"):
                path = source / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(name)
            module.collect(source, output)
            for name in names:
                self.assertEqual((output / name).read_text(), name)
            self.assertFalse((output / "openvino/libs/libopenvino.so").exists())

    def test_missing_license_fails(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaises(RuntimeError):
                module.collect(root, root / "output")
