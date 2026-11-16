#!/usr/bin/env bash
# 러스트 판 Mac 앱 포장 — 유니버설 실행 파일 하나를 LogiFlowRs.app 으로 감싸고 dmg · zip · SHA256SUMS 를 만든다.
# macOS 러너(또는 로컬 macOS)에서 실행. 서명은 ad-hoc(-) 만 — Apple 개발자 서명·공증 없음.
#
# 사용: make-app.sh <유니버설 실행 파일> <버전> <출력 폴더>
# Swift 판(LogiFlow.app · com.happy4ed.logiflow)과 나란히 설치되도록 번들 이름·id 를 따로 둔다.
set -euo pipefail

EXE="$1"
VERSION="$2"
OUT="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
APP_NAME="LogiFlowRs"
BUNDLE_ID="com.happy4ed.logiflow.rs"

mkdir -p "$OUT"
WORK="$(mktemp -d)"
APP="$WORK/${APP_NAME}.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$EXE" "$APP/Contents/MacOS/${APP_NAME}"
chmod +x "$APP/Contents/MacOS/${APP_NAME}"

# 앱 아이콘 — Windows 판과 같은 「두 화면 흐름」 마크(logiflow.ico 안 PNG)로 icns.
ICONSET="$WORK/AppIcon.iconset"
mkdir -p "$ICONSET"
python3 - "$ROOT/LogiFlow/Resources/logiflow.ico" "$WORK" <<'PY'
import struct, sys, os
ico, work = sys.argv[1], sys.argv[2]
d = open(ico, 'rb').read()
n = struct.unpack('<H', d[4:6])[0]
for i in range(n):
    e = d[6 + 16 * i:22 + 16 * i]
    w = e[0] or 256
    size, off = struct.unpack('<II', e[8:16])
    open(os.path.join(work, 'src%d.png' % w), 'wb').write(d[off:off + size])
PY
cp "$WORK/src16.png"  "$ICONSET/icon_16x16.png"
cp "$WORK/src32.png"  "$ICONSET/icon_16x16@2x.png"
cp "$WORK/src32.png"  "$ICONSET/icon_32x32.png"
cp "$WORK/src64.png"  "$ICONSET/icon_32x32@2x.png"
cp "$WORK/src128.png" "$ICONSET/icon_128x128.png"
cp "$WORK/src256.png" "$ICONSET/icon_128x128@2x.png"
cp "$WORK/src256.png" "$ICONSET/icon_256x256.png"
sips -z 512 512   "$WORK/src256.png" --out "$ICONSET/icon_256x256@2x.png" >/dev/null
sips -z 512 512   "$WORK/src256.png" --out "$ICONSET/icon_512x512.png" >/dev/null
sips -z 1024 1024 "$WORK/src256.png" --out "$ICONSET/icon_512x512@2x.png" >/dev/null
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>LogiFlow</string>
  <key>CFBundleDisplayName</key><string>LogiFlow</string>
  <key>CFBundleIdentifier</key><string>${BUNDLE_ID}</string>
  <key>CFBundleExecutable</key><string>${APP_NAME}</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>CFBundleDevelopmentRegion</key><string>ko</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
  <key>NSHumanReadableCopyright</key><string>Copyright (c) 2026 happy4ed · MIT</string>
</dict>
</plist>
PLIST
printf 'APPL????' > "$APP/Contents/PkgInfo"

# ad-hoc 서명(Apple Silicon 은 서명 없는 실행 파일을 띄우지 않는다). 개발자 서명·공증은 없음.
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"

BASE="LogiFlow-v${VERSION}-mac"
rm -f "$OUT/${BASE}.zip" "$OUT/${BASE}.dmg"
( cd "$WORK" && ditto -c -k --sequesterRsrc --keepParent "${APP_NAME}.app" "$OUT/${BASE}.zip" )

STAGE="$WORK/dmg"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "LogiFlow ${VERSION}" -srcfolder "$STAGE" -ov -format UDZO "$OUT/${BASE}.dmg" >/dev/null
cp "$OUT/${BASE}.dmg" "$OUT/LogiFlow-mac.dmg"

( cd "$OUT" && shasum -a 256 "${BASE}.dmg" "LogiFlow-mac.dmg" "${BASE}.zip" > "${BASE}-SHA256SUMS.txt" && cat "${BASE}-SHA256SUMS.txt" )
# 시험 단계가 쓰도록 .app 도 남긴다(포장물에는 dmg · zip 만).
rm -rf "$OUT/../app-under-test" && mkdir -p "$OUT/../app-under-test" && cp -R "$APP" "$OUT/../app-under-test/"
rm -rf "$WORK"
ls -la "$OUT"
