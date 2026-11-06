#!/usr/bin/env bash
set -euo pipefail
irodori_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
irodori_runtime="$irodori_root/.runtime"
if [[ -d "$irodori_runtime/openvino" ]]; then
  export IRODORI_OPENVINO_LIBRARY="${IRODORI_OPENVINO_LIBRARY:-$irodori_runtime/openvino/libopenvino_c.so}"
fi
if [[ -d "$irodori_runtime/intel/usr/lib/x86_64-linux-gnu" ]]; then
  export LD_LIBRARY_PATH="$irodori_runtime/intel/usr/lib/x86_64-linux-gnu:$irodori_runtime/intel/usr/lib/x86_64-linux-gnu/intel-opencl:/usr/lib/wsl/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
  export OCL_ICD_VENDORS="$irodori_runtime/intel/icd"
fi
if [[ $# == 0 ]]; then
  echo "Usage: scripts/with-openvino.sh COMMAND [ARGS...]" >&2
  exit 2
fi
exec "$@"
