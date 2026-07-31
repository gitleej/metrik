export const GLASS_MODES = Object.freeze({
  off: "off",
  css: "css",
  native: "native",
  alpha: "alpha",
});

const USER_GLASS_TINTS = Object.freeze(["dark", "light", "clear"]);

export function nextGlassTint(tintStyle) {
  const index = USER_GLASS_TINTS.indexOf(tintStyle);
  return USER_GLASS_TINTS[(index + 1) % USER_GLASS_TINTS.length];
}

export function resolveGlassMode({
  enabled,
  tintStyle = "dark",
  nativeAvailable = false,
  trueAlphaAvailable = false,
}) {
  if (!enabled) return GLASS_MODES.off;
  if (trueAlphaAvailable) return GLASS_MODES.alpha;
  return nativeAvailable ? GLASS_MODES.native : GLASS_MODES.css;
}

export function resolveWindowsGlassComposition({
  enabled,
  tintStyle = "dark",
}) {
  return {
    mode: resolveGlassMode({
      enabled,
      tintStyle,
      nativeAvailable: false,
      trueAlphaAvailable: true,
    }),
    resetWebviewBackground: false,
    mutateNativeBackdrop: false,
  };
}

export function glassShellAppearance(
  kind,
  {
    transparent = false,
    glassMode = GLASS_MODES.css,
    glassTint = "dark",
    glassAlpha = 0.82,
    isMac = false,
    vertical = false,
    loading = false,
  } = {},
) {
  if (kind !== "widget" && kind !== "strip") {
    throw new TypeError(`Unsupported glass shell kind: ${kind}`);
  }

  const prefix = kind === "widget" ? "widget-shell" : "strip-shell";
  const classes = [prefix];
  if (kind === "strip" && vertical) classes.push(`${prefix}--vertical`);
  if (transparent) {
    classes.push(`${prefix}--transparent`);
    if (glassMode === GLASS_MODES.css) classes.push(`${prefix}--glass-css`);
    if (!isMac && glassTint === "light") classes.push(`${prefix}--glass-light`);
    if (!isMac && glassTint === "clear") classes.push(`${prefix}--glass-clear`);
  }
  if (isMac) classes.push(`${prefix}--mac`);
  if (kind === "widget" && loading) classes.push("is-loading");

  const edgeInteractive =
    transparent
    && !isMac
    && glassTint === "clear";
  const trueAlpha =
    edgeInteractive
    && glassMode === GLASS_MODES.alpha;

  return {
    className: classes.join(" "),
    style: transparent
      ? {
          "--glass-alpha": glassAlpha,
        }
      : {},
    edgeInteractive,
    trueAlpha,
  };
}
