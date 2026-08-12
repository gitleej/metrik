import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";

const tauriConfig = JSON.parse(fs.readFileSync("src-tauri/tauri.conf.json", "utf8"));
const widgetInfo = fs.readFileSync("WidgetExtension/Info.plist", "utf8");

test("macOS release embeds the native WidgetKit extension and helpers", () => {
  assert.equal(
    tauriConfig.build.beforeBundleCommand,
    "node scripts/prepare-macos-widget.mjs",
  );
  assert.deepEqual(tauriConfig.bundle.macOS.files, {
    "PlugIns/MetrikWidget.appex":
      "../.build/macos-widget-extension/MetrikWidget.appex",
    "Helpers/metrik-widget-reload":
      "../.build/macos-widget-extension/Helpers/metrik-widget-reload",
  });
});

test("production widget uses the host app bundle namespace", () => {
  assert.match(widgetInfo, /<string>app\.metrik\.desktop\.widget<\/string>/);
  assert.doesNotMatch(widgetInfo, /widget-preview/);
});
