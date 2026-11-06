#!/usr/bin/env bash
# Check shipped runtime libraries and model paths from outside the repository.
set -euo pipefail
target=${1:?Usage: smoke-package.sh TARGET}
bundle="$PWD/target/$target/release/bundle"
work=$(mktemp -d)
mounted=false
cleanup() {
  if [[ "$mounted" == true ]]; then hdiutil detach "$work/mount"; fi
  rm -rf "$work"
}
trap cleanup EXIT
unset IRODORU_MODELS IRODORU_TEST_MODELS LD_LIBRARY_PATH DYLD_LIBRARY_PATH
case "$target" in
  x86_64-pc-windows-msvc)
    archive=$(find "$bundle" -name '*.zip' -type f -print -quit)
    7z x "$archive" "-o$work/package" >/dev/null
    (cd "$work" && ./package/irodoru.exe --smoke-test)
    ;;
  aarch64-apple-darwin)
    archive=$(find "$bundle" -name '*.dmg' -type f -print -quit)
    mkdir "$work/mount"
    # cargo-packager embeds our MIT license in the DMG; hdiutil asks before mounting.
    hdiutil attach -readonly -nobrowse -mountpoint "$work/mount" "$archive" <<<'Y'
    mounted=true
    (cd "$work" && ./mount/irodoru.app/Contents/MacOS/irodoru --smoke-test)
    ;;
  x86_64-unknown-linux-gnu)
    archive=$(find "$bundle" -name '*.AppImage' -type f -print -quit)
    (cd "$work" && "$archive" --appimage-extract >/dev/null)
    (cd "$work" && APPDIR="$work/squashfs-root" ./squashfs-root/AppRun --smoke-test)
    rm -rf "$work/squashfs-root"
    archive=$(find "$bundle" -name '*.deb' -type f -print -quit)
    dpkg-deb --extract "$archive" "$work/package"
    (cd "$work" && ./package/usr/bin/irodoru --smoke-test)
    ;;
  *) echo "Unsupported desktop target: $target" >&2; exit 1 ;;
esac
