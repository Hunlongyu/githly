#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
platform=${1:?Expected linux-x64, linux-x64-appimage, macos-x64 or macos-arm64}
version=$(cargo metadata --locked --no-deps --format-version 1 | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="githly"))')
if [[ ${GITHUB_REF_TYPE:-} == tag && ${GITHUB_REF_NAME:-} != "v$version" ]]; then
    echo "Git tag must match Cargo version $version" >&2
    exit 1
fi
out="$PWD/dist/$platform"
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
mkdir -p "$out"
binary=target/release/githly
case "$platform:$(uname -sm)" in
    'linux-x64:Linux x86_64')
        name="Githly-$version-$platform.deb"
        root="$stage/package"
        install -Dm755 "$binary" "$root/usr/bin/githly"
        install -Dm644 assets/branding/githly-256.png "$root/usr/share/icons/hicolor/256x256/apps/githly.png"
        install -Dm644 assets/linux/githly.desktop "$root/usr/share/applications/githly.desktop"
        mkdir -p "$root/DEBIAN" "$root/usr/share/doc/githly" "$stage/debian"
        cp README.md README.zh-CN.md vendor/gpui-component/LICENSE-APACHE "$root/usr/share/doc/githly/"
        touch "$stage/debian/control"
        libraries=$(cd "$stage" && dpkg-shlibdeps -O -e"$root/usr/bin/githly")
        cat > "$root/DEBIAN/control" <<EOF
Package: githly
Version: $version
Architecture: amd64
Maintainer: Hunlongyu <Hunlongyu@users.noreply.github.com>
Depends: ${libraries#shlibs:Depends=}, git, xdg-utils, libvulkan1, libxkbcommon-x11-0
Recommends: mesa-vulkan-drivers, xdg-desktop-portal
Description: Desktop Git repository browser
EOF
        dpkg-deb --root-owner-group --build "$root" "$out/$name"
        ;;
    'linux-x64-appimage:Linux x86_64')
        name="Githly-$version-linux-x64.AppImage"
        appdir="$stage/Githly.AppDir"
        mkdir -p "$appdir/usr/share/doc/githly"
        cp vendor/gpui-component/LICENSE-APACHE "$appdir/usr/share/doc/githly/"
        cp assets/branding/githly-256.png "$stage/githly.png"
        curl --fail --location --retry 3 --output "$stage/linuxdeploy" \
            https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-x86_64.AppImage
        curl --fail --location --retry 3 --output "$stage/runtime" \
            https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64
        (cd "$stage" && sha256sum --check <<'SUMS'
c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d  linuxdeploy
2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d  runtime
SUMS
        )
        chmod +x "$stage/linuxdeploy"
        export APPIMAGE_EXTRACT_AND_RUN=1 LDAI_RUNTIME_FILE="$stage/runtime"
        export LDAI_OUTPUT="$out/$name" LDAI_VERSION="$version"
        "$stage/linuxdeploy" --appdir "$appdir" --executable "$binary" \
            --desktop-file assets/linux/githly.desktop --icon-file "$stage/githly.png" --output appimage
        ;;
    'macos-arm64:Darwin arm64'|'macos-x64:Darwin x86_64')
        name="Githly-$version-$platform.dmg"
        volume="$stage/volume"
        app="$volume/Githly.app"
        mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$stage/Githly.iconset"
        install -m755 "$binary" "$app/Contents/MacOS/githly"
        for size in 16 32 128 256 512; do
            for scale in 1 2; do
                side=$((size * scale))
                suffix=''
                if [[ $scale == 2 ]]; then suffix='@2x'; fi
                sips -s format png -z "$side" "$side" assets/branding/githly-256.png --out "$stage/Githly.iconset/icon_${size}x${size}${suffix}.png" >/dev/null
            done
        done
        iconutil -c icns "$stage/Githly.iconset" -o "$app/Contents/Resources/Githly.icns"
        sed "s/@VERSION@/$version/g" assets/macos/Info.plist > "$app/Contents/Info.plist"
        cp README.md README.zh-CN.md vendor/gpui-component/LICENSE-APACHE "$app/Contents/Resources/"
        plutil -lint "$app/Contents/Info.plist"
        codesign --force --sign - "$app"
        codesign --verify --strict "$app"
        ln -s /Applications "$volume/Applications"
        hdiutil create -quiet -ov -format UDZO -volname Githly -srcfolder "$volume" "$out/$name"
        ;;
    *) echo "Unsupported platform or build host: $platform ($(uname -sm))" >&2; exit 1 ;;
esac
python3 - "$out" "$name" "$platform" "$version" <<'PY'
import hashlib, json, pathlib, sys
out, name, platform, version = sys.argv[1:]
out = pathlib.Path(out)
with (out / name).open("rb") as stream:
    sha = hashlib.sha256()
    for block in iter(lambda: stream.read(1024 * 1024), b""):
        sha.update(block)
    digest = sha.hexdigest()
base = "https://github.com/Hunlongyu/githly/releases"
manifest = {"version": version, "notes_url": f"{base}/tag/v{version}", "installer": {
    "url": f"{base}/download/v{version}/{name}", "size": (out / name).stat().st_size, "sha256": digest}}
for section in pathlib.Path("CHANGELOG.md").read_text(encoding="utf-8").split("\n## "):
    heading, _, body = section.partition("\n")
    if heading.startswith(f"v{version} · "):
        manifest["notes"] = body.strip()
        break
(out / f"update-{platform}.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
PY
