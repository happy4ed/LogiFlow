#!/usr/bin/env bash
# SPM 으로 빌드한 실행 파일을 .app 번들로 감싸고 zip·dmg 로 포장한다.
# macOS 러너(또는 로컬 macOS)에서 실행. 리눅스에서는 macOS 빌드 불가.
#
# 사용: scripts/make-app.sh [버전]
#   버전 생략 시 mac/VERSION 을 읽는다.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"   # mac/ 디렉터리
cd "$HERE"

VERSION="${1:-$(cat VERSION | tr -d '[:space:]')}"
APP_NAME="LogiFlow"
EXE_NAME="LogiFlowDiag"
BUNDLE_ID="com.happy4ed.logiflow.diag"

echo "== LogiFlow mac 진단 빌드 v${VERSION} =="

# 1) 릴리스 빌드
swift build -c release

BIN_PATH="$(swift build -c release --show-bin-path)"
EXE="${BIN_PATH}/${EXE_NAME}"
if [[ ! -f "$EXE" ]]; then
  echo "빌드 산출물 없음: $EXE" >&2
  exit 1
fi

# 2) .app 번들 조립
DIST="dist"
APP_DIR="${DIST}/${APP_NAME}.app"
rm -rf "$APP_DIR"
mkdir -p "${APP_DIR}/Contents/MacOS" "${APP_DIR}/Contents/Resources"
cp "$EXE" "${APP_DIR}/Contents/MacOS/${APP_NAME}"
chmod +x "${APP_DIR}/Contents/MacOS/${APP_NAME}"

cat > "${APP_DIR}/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>${APP_NAME}</string>
  <key>CFBundleDisplayName</key><string>${APP_NAME} 진단</string>
  <key>CFBundleIdentifier</key><string>${BUNDLE_ID}</string>
  <key>CFBundleExecutable</key><string>${APP_NAME}</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
</dict>
</plist>
PLIST

printf 'APPL????' > "${APP_DIR}/Contents/PkgInfo"

# 3) zip (가장 호환성 높은 배포 형태)
ZIP="${DIST}/${APP_NAME}-v${VERSION}-mac.zip"
rm -f "$ZIP"
( cd "$DIST" && ditto -c -k --sequesterRsrc --keepParent "${APP_NAME}.app" "$(basename "$ZIP")" )

# 4) dmg (가능하면). hdiutil 은 macOS 전용.
DMG="${DIST}/${APP_NAME}-v${VERSION}-mac.dmg"
rm -f "$DMG"
if command -v hdiutil >/dev/null 2>&1; then
  STAGE="$(mktemp -d)"
  cp -R "$APP_DIR" "$STAGE/"
  hdiutil create -volname "${APP_NAME} ${VERSION}" -srcfolder "$STAGE" -ov -format UDZO "$DMG" >/dev/null
  rm -rf "$STAGE"
  echo "dmg: $DMG"
else
  echo "hdiutil 없음 — dmg 생략(zip 만 생성)"
fi

# 5) 체크섬
SUMS="${DIST}/${APP_NAME}-v${VERSION}-mac-SHA256SUMS.txt"
( cd "$DIST" && shasum -a 256 "${APP_NAME}-v${VERSION}-mac.zip" $( [[ -f "${APP_NAME}-v${VERSION}-mac.dmg" ]] && echo "${APP_NAME}-v${VERSION}-mac.dmg" ) > "$(basename "$SUMS")" )

echo "== 완료 =="
echo "app:  $APP_DIR"
echo "zip:  $ZIP"
echo "sums: $SUMS"
