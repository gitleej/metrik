import assert from "node:assert/strict";
import test from "node:test";

import {
  GLASS_MODES,
  glassShellAppearance,
  nextGlassTint,
  resolveGlassMode,
  resolveWindowsGlassComposition,
} from "./glassAppearance.js";

test("the user-facing component appearance cycles through exactly three tints", () => {
  assert.equal(nextGlassTint("dark"), "light");
  assert.equal(nextGlassTint("light"), "clear");
  assert.equal(nextGlassTint("clear"), "dark");
  assert.equal(nextGlassTint("off"), "dark");
});

test("Windows glass keeps one alpha composition mode for every tint", () => {
  for (const tintStyle of ["dark", "light", "clear"]) {
    assert.equal(
      resolveGlassMode({
        enabled: true,
        tintStyle,
        nativeAvailable: false,
        trueAlphaAvailable: true,
      }),
      GLASS_MODES.alpha,
    );
  }
});

test("Windows glass transitions never reset WebView or mutate the native backdrop", () => {
  for (const state of [
    { enabled: true, tintStyle: "clear" },
    { enabled: false, tintStyle: "clear" },
    { enabled: true, tintStyle: "dark" },
    { enabled: true, tintStyle: "light" },
    { enabled: true, tintStyle: "clear" },
  ]) {
    const decision = resolveWindowsGlassComposition(state);
    assert.equal(
      decision.mode,
      state.enabled ? GLASS_MODES.alpha : GLASS_MODES.off,
    );
    assert.equal(decision.resetWebviewBackground, false);
    assert.equal(decision.mutateNativeBackdrop, false);
  }
});

test("every Windows tint keeps alpha classes free of the CSS fallback", () => {
  for (const glassTint of ["dark", "light", "clear"]) {
    const appearance = glassShellAppearance("widget", {
      transparent: true,
      glassMode: GLASS_MODES.alpha,
      glassTint,
    });

    assert.doesNotMatch(appearance.className, /--glass-css/);
    assert.equal(appearance.style["--glass-alpha"], 0.82);
    assert.equal(appearance.trueAlpha, glassTint === "clear");
  }
});

test("compact and strip clear glass share one true-alpha appearance", () => {
  for (const kind of ["widget", "strip"]) {
    const appearance = glassShellAppearance(kind, {
      transparent: true,
      glassMode: GLASS_MODES.alpha,
      glassTint: "clear",
      glassAlpha: 0.82,
    });
    const prefix = kind === "widget" ? "widget-shell" : "strip-shell";

    assert.equal(appearance.trueAlpha, true);
    assert.equal(appearance.edgeInteractive, true);
    assert.match(appearance.className, new RegExp(`${prefix}--transparent`));
    assert.match(appearance.className, new RegExp(`${prefix}--glass-clear`));
    assert.doesNotMatch(appearance.className, new RegExp(`${prefix}--glass-light`));
    assert.doesNotMatch(appearance.className, /--glass-css/);
    assert.deepEqual(appearance.style, {
      "--glass-alpha": 0.82,
    });
    assert.equal(
      Object.keys(appearance.style).some((key) => key.startsWith("--wall-")),
      false,
    );
  }
});

test("browser clear fallback keeps the edge interaction without claiming true alpha", () => {
  const appearance = glassShellAppearance("widget", {
    transparent: true,
    glassMode: GLASS_MODES.css,
    glassTint: "clear",
  });

  assert.equal(appearance.edgeInteractive, true);
  assert.equal(appearance.trueAlpha, false);
  assert.match(appearance.className, /widget-shell--glass-css/);
  assert.doesNotMatch(appearance.className, /widget-shell--glass-light/);
});

test("macOS ignores a stored Windows clear tint", () => {
  const appearance = glassShellAppearance("widget", {
    transparent: true,
    glassMode: GLASS_MODES.native,
    glassTint: "clear",
    isMac: true,
  });

  assert.equal(appearance.edgeInteractive, false);
  assert.equal(appearance.trueAlpha, false);
  assert.match(appearance.className, /widget-shell--mac/);
  assert.doesNotMatch(appearance.className, /widget-shell--glass-(?:light|clear)/);
});
