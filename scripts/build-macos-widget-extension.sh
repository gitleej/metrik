#!/bin/zsh
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  exit 0
fi

SCRIPT_DIR="${0:A:h}"
PROJECT_DIR="${SCRIPT_DIR:h}"
BUILD_DIR="$PROJECT_DIR/.build/macos-widget-extension"
APPEX_BUNDLE="$BUILD_DIR/MetrikWidget.appex"
APPEX_BINARY="$APPEX_BUNDLE/Contents/MacOS/MetrikWidget"
APPEX_RESOURCES="$APPEX_BUNDLE/Contents/Resources"
HELPERS_DIR="$BUILD_DIR/Helpers"
SDK_PATH="$(xcrun --sdk macosx --show-sdk-path)"
APP_VERSION="$(node -p "require('$PROJECT_DIR/package.json').version")"
APP_BUILD_NUMBER="${GITHUB_RUN_NUMBER:-1}"

# A universal Tauri release needs a universal extension too. Local arm64/x86_64
# builds may override this to one architecture to keep iteration fast.
if [[ -n "${METRIK_WIDGET_ARCHS:-}" ]]; then
  ARCHS=(${=METRIK_WIDGET_ARCHS})
elif [[ "${TAURI_ENV_ARCH:-}" == "universal" ]]; then
  ARCHS=(arm64 x86_64)
else
  ARCHS=("$(uname -m)")
fi

rm -rf "$BUILD_DIR"
mkdir -p "$APPEX_BUNDLE/Contents/MacOS" "$APPEX_RESOURCES" "$HELPERS_DIR" "$BUILD_DIR/arch"

build_universal() {
  local output="$1"
  shift
  local name="${output:t}"
  local binaries=()
  local arch
  for arch in "${ARCHS[@]}"; do
    local arch_output="$BUILD_DIR/arch/$name-$arch"
    xcrun swiftc \
      -parse-as-library \
      -O \
      -target "${arch}-apple-macosx14.0" \
      -sdk "$SDK_PATH" \
      "$@" \
      -o "$arch_output"
    binaries+=("$arch_output")
  done
  if (( ${#binaries[@]} == 1 )); then
    ditto "${binaries[1]}" "$output"
  else
    xcrun lipo -create "${binaries[@]}" -output "$output"
  fi
  chmod +x "$output"
}

echo "Building Metrik WidgetKit extension for ${ARCHS[*]}..."
build_universal "$APPEX_BINARY" \
  -application-extension \
  -framework AppKit \
  -framework SwiftUI \
  -framework WidgetKit \
  -Xlinker -e \
  -Xlinker _NSExtensionMain \
  "$PROJECT_DIR/WidgetExtension/Sources/MetrikWidgetModels.swift" \
  "$PROJECT_DIR/WidgetExtension/Sources/MetrikWidgetViews.swift" \
  "$PROJECT_DIR/WidgetExtension/Sources/MetrikWidgetBundle.swift"

ditto "$PROJECT_DIR/WidgetExtension/Info.plist" "$APPEX_BUNDLE/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $APP_VERSION" "$APPEX_BUNDLE/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $APP_BUILD_NUMBER" "$APPEX_BUNDLE/Contents/Info.plist"

for asset in \
  chatgpt-app-icon.png \
  claude-app-icon.jpg \
  zcode-app-icon.png \
  opencode-app-icon.png \
  kimi-app-icon.png \
  antigravity-app-icon.png
do
  ditto "$PROJECT_DIR/src/assets/$asset" "$APPEX_RESOURCES/$asset"
done

build_universal "$HELPERS_DIR/metrik-widget-publish" \
  -framework Foundation \
  "$PROJECT_DIR/WidgetExtension/Sources/MetrikWidgetPublisher.swift"
build_universal "$HELPERS_DIR/metrik-widget-reload" \
  -framework WidgetKit \
  "$PROJECT_DIR/WidgetExtension/Sources/MetrikWidgetReloader.swift"

# This is ad-hoc bundle integrity, not Developer ID distribution signing. The
# nested extension must be sealed before Tauri seals the outer app bundle.
# Everything that touches the App Group container must carry BOTH app-sandbox
# and the group entitlement: a non-sandboxed binary accessing the container is
# what triggers macOS to keep prompting about cross-app data access.
codesign --force --sign - \
  --entitlements "$PROJECT_DIR/WidgetExtension/MetrikWidget.entitlements" \
  "$APPEX_BUNDLE"
codesign --force --sign - \
  --entitlements "$PROJECT_DIR/WidgetExtension/MetrikWidgetPublisher.entitlements" \
  "$HELPERS_DIR/metrik-widget-publish"
codesign --force --sign - "$HELPERS_DIR/metrik-widget-reload"
codesign --verify --strict --verbose=2 "$APPEX_BUNDLE"

echo "Prepared WidgetKit bundle: $APPEX_BUNDLE"
