#!/usr/bin/env bash
# Check that packages include licenses and exclude model weights.
set -euo pipefail
target=${1:?Usage: check-package.sh TARGET}
bundle="$PWD/target/$target/release/bundle"
work=$(mktemp -d)
mounted=false
cleanup() {
  if [[ "$mounted" == true ]]; then hdiutil detach "$work/mount"; fi
  rm -rf "$work"
}
trap cleanup EXIT
check_package() {
  if [[ -n $(find "$1" -type f \( -name '*.onnx' -o -name '*.onnx.*' -o -name 'bundle.json' \) -print -quit) ]]; then
    echo "::error::App package contains model weights: $1" >&2
    exit 1
  fi
  local license_dir
  license_dir=$(find "$1" -type f -name THIRD-PARTY-NOTICES.txt -print -quit)
  if [[ -z "$license_dir" ]]; then
    echo "::error::App package is missing third-party licenses: $1" >&2
    exit 1
  fi
  license_dir=$(dirname "$license_dir")
  test -s "$license_dir/LICENSE"
  test -s "$license_dir/THIRD-PARTY-SOURCES.txt"
  test -s "$license_dir/model/ETHICAL-RESTRICTIONS.txt"
  test -s "$license_dir/sources/symphonia-0.6.1.crate"
  test -s "$license_dir/sources/option-ext-0.2.0.crate"
}
case "$target" in
  x86_64-pc-windows-msvc)
    archive=$(find "$bundle" -name '*.zip' -type f -print -quit)
    7z x "$archive" "-o$work/package" >/dev/null
    check_package "$work/package"
    ;;
  aarch64-apple-darwin)
    archive=$(find "$bundle" -name '*.dmg' -type f -print -quit)
    mkdir "$work/mount"
    # cargo-packager embeds our MIT license in the DMG; hdiutil asks before mounting.
    hdiutil attach -readonly -nobrowse -mountpoint "$work/mount" "$archive" <<<'Y'
    mounted=true
    check_package "$work/mount"
    ;;
  x86_64-unknown-linux-gnu)
    archive=$(find "$bundle" -name '*.AppImage' -type f -print -quit)
    (cd "$work" && "$archive" --appimage-extract >/dev/null)
    check_package "$work/squashfs-root"
    rm -rf "$work/squashfs-root"
    archive=$(find "$bundle" -name '*.deb' -type f -print -quit)
    dpkg-deb --extract "$archive" "$work/package"
    check_package "$work/package"
    ;;
  *) echo "Unsupported desktop target: $target" >&2; exit 1 ;;
esac
