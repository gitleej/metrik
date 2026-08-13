import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";

const tauriConfig = JSON.parse(fs.readFileSync("src-tauri/tauri.conf.json", "utf8"));
const widgetInfo = fs.readFileSync("WidgetExtension/Info.plist", "utf8");
const widgetViews = fs.readFileSync(
  "WidgetExtension/Sources/MetrikWidgetViews.swift",
  "utf8",
);
const widgetModels = fs.readFileSync(
  "WidgetExtension/Sources/MetrikWidgetModels.swift",
  "utf8",
);
const reloaderInfo = fs.readFileSync(
  "WidgetExtension/MetrikWidgetReloader.Info.plist",
  "utf8",
);
const reloaderSource = fs.readFileSync(
  "WidgetExtension/Sources/MetrikWidgetReloader.swift",
  "utf8",
);
const publisherInfo = fs.readFileSync(
  "WidgetExtension/MetrikWidgetPublisher.Info.plist",
  "utf8",
);
const widgetBuildScript = fs.readFileSync(
  "scripts/build-macos-widget-extension.sh",
  "utf8",
);

test("macOS release embeds the native WidgetKit extension and helpers", () => {
  assert.equal(
    tauriConfig.build.beforeBundleCommand,
    "node scripts/prepare-macos-widget.mjs",
  );
  assert.deepEqual(tauriConfig.bundle.macOS.files, {
    "PlugIns/MetrikWidget.appex":
      "../.build/macos-widget-extension/MetrikWidget.appex",
    "Helpers/metrik-widget-publish":
      "../.build/macos-widget-extension/Helpers/metrik-widget-publish",
    "Helpers/metrik-widget-reload":
      "../.build/macos-widget-extension/Helpers/metrik-widget-reload",
  });
});

test("production widget uses the host app bundle namespace", () => {
  assert.match(widgetInfo, /<string>app\.metrik\.desktop\.widget<\/string>/);
  assert.doesNotMatch(widgetInfo, /widget-preview/);
  assert.match(publisherInfo, /<string>app\.metrik\.desktop\.widget<\/string>/);
  assert.doesNotMatch(
    publisherInfo,
    /<key>CFBundleIdentifier<\/key>\s*<string>[^<]*widget-publish/,
  );
});

test("production widget never substitutes gallery preview data", () => {
  assert.doesNotMatch(
    widgetViews,
    /MetrikWidgetStore\.load\(\)\s*\?\?\s*MetrikWidgetStore\.preview/,
  );
  assert.match(
    widgetViews,
    /MetrikWidgetStore\.load\(\)\s*\?\?\s*MetrikWidgetStore\.unavailable/,
  );
  assert.doesNotMatch(widgetModels, /forResource:\s*"preview-widget-snapshot"/);
  assert.doesNotMatch(widgetModels, /Data\(contentsOf:/);
  assert.match(widgetModels, /UserDefaults\.standard/);
  assert.match(widgetModels, /Shared snapshot decoded with/);
});

test("timeline reloader identifies as the containing Metrik app", () => {
  assert.match(reloaderInfo, /<string>app\.metrik\.desktop<\/string>/);
  assert.match(reloaderSource, /RunLoop\.current\.run/);
});

test("production widget uses the containing app build version", () => {
  assert.match(widgetBuildScript, /APP_BUILD_NUMBER="\$APP_VERSION"/);
  assert.doesNotMatch(
    widgetBuildScript,
    /APP_BUILD_NUMBER=.*(?:date|GITHUB_RUN_NUMBER)/,
  );
});
