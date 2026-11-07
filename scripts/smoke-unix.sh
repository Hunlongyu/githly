#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
platform=${1:?Missing platform}
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
git init -q "$stage/repo"
case "$platform" in
    linux-x64)
        dpkg-deb -x dist/"$platform"/*.deb "$stage/root"
        export XDG_RUNTIME_DIR="$stage/runtime" XDG_CONFIG_HOME="$stage/config" XDG_DATA_HOME="$stage/data"
        mkdir -m 700 "$XDG_RUNTIME_DIR"
        xvfb-run -a dbus-run-session -- bash -eu -c '
            "$1" "$2" &
            pid=$!
            trap "kill $pid 2>/dev/null || true" EXIT
            for ((i=0; i<60; i++)); do
                kill -0 "$pid"
                if xdotool search --onlyvisible --name "^Githly$" >/dev/null; then exit 0; fi
                sleep 1
            done
            exit 1
        ' smoke "$stage/root/usr/bin/githly" "$stage/repo"
        ;;
    macos-*)
        hdiutil attach -quiet -readonly -nobrowse -mountpoint "$stage/mount" dist/"$platform"/*.dmg
        trap 'hdiutil detach -quiet "$stage/mount"; rm -rf -- "$stage"' EXIT
        ditto "$stage/mount/Githly.app" "$stage/Githly.app"
        codesign --verify --strict "$stage/Githly.app"
        # Metal is not exposed on every virtual Mac. Test the window when it is available.
        if swift -e 'import Metal; import Darwin; exit(MTLCreateSystemDefaultDevice() == nil ? 1 : 0)'; then
            "$stage/Githly.app/Contents/MacOS/githly" "$stage/repo" &
            pid=$!
            trap 'kill "$pid" 2>/dev/null || true; hdiutil detach -quiet "$stage/mount"; rm -rf -- "$stage"' EXIT
            swift - "$pid" <<'SWIFT'
import CoreGraphics
import Foundation
let pid = Int(CommandLine.arguments[1])!
for _ in 0..<60 {
    let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
    if windows.contains(where: { $0[kCGWindowOwnerPID as String] as? Int == pid && $0[kCGWindowLayer as String] as? Int == 0 }) { exit(0) }
    Thread.sleep(forTimeInterval: 1)
}
exit(1)
SWIFT
        else
            echo "::notice::No Metal device on this runner; verified the app bundle and ran headless UI tests."
        fi
        ;;
    *) exit 1 ;;
esac
