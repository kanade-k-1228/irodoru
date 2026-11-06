#!/usr/bin/env bash
set -euo pipefail
irodori_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$irodori_root"
if [[ $# -gt 1 || ( $# == 1 && "$1" != "--wsl" ) ]]; then
  echo "Usage: scripts/setup-openvino.sh [--wsl]" >&2
  exit 2
fi
command -v uv >/dev/null || { echo "Install uv before running this script." >&2; exit 1; }
uv venv --python 3.12 --allow-existing .runtime/setup-venv
uv pip install --python .runtime/setup-venv/bin/python 'openvino==2026.4.1' 'onnx==1.23.2'
.runtime/setup-venv/bin/python - <<'PY'
import importlib.util
import shutil
from pathlib import Path
package = Path(importlib.util.find_spec('openvino').origin).parent
destination = Path('.runtime/openvino')
shutil.copytree(package / 'libs', destination, dirs_exist_ok=True)
c_library, = destination.glob('libopenvino_c.so.*')
link = destination / 'libopenvino_c.so'
if link.is_symlink() or link.exists():
    link.unlink()
link.symlink_to(c_library.name)
print('Installed OpenVINO in', destination.resolve())
PY
if [[ "${1:-}" == "--wsl" ]]; then
  test -e /dev/dxg || { echo "WSL GPU device /dev/dxg is missing." >&2; exit 1; }
  mkdir -p .runtime/packages .runtime/intel/icd
  (
    cd .runtime/packages
    apt-get download intel-opencl-icd libze-intel-gpu1 libze1 ocl-icd-libopencl1 clinfo libigdgmm12 libigc2 libigdfcl2
  )
  for package in .runtime/packages/*.deb; do dpkg-deb -x "$package" .runtime/intel; done
  printf '%s\n' "$irodori_root/.runtime/intel/usr/lib/x86_64-linux-gnu/intel-opencl/libigdrcl.so" > .runtime/intel/icd/intel.icd
  scripts/with-openvino.sh .runtime/intel/usr/bin/clinfo -l
fi
echo 'Next: .runtime/setup-venv/bin/python pkgs/engine/script/portable_onnx.py --model-dir models'
echo 'Then: scripts/with-openvino.sh target/release/irodori-server devices'
