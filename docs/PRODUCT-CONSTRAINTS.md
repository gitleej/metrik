# Metrik product constraints

These are public, durable constraints that affect product correctness. They
apply to every implementation regardless of which tool or contributor makes the
change.

## Data truth and privacy

- Official quota, locally parsed usage, and estimated cost are different facts.
  Keep them visibly separate and never present an estimate as official billing.
- Never synthesize a comparison curve or replace a failed desktop read with demo
  numbers. Missing and stale data must be labeled explicitly.
- Manual refresh requests `usage_snapshot` with `force: true`, bypassing quota
  TTL caches. A failed refresh retains the last rows and marks them stale; it
  never clears them silently.
- Metrik is local-first. Optional multi-device sync must not upload prompts,
  conversation text, credentials, or raw tool output.
- The compact widget prioritizes per-agent official quota windows, including
  remaining percentage and reset countdown. Token analytics belong in the
  expanded view.
- The strip prefers the five-hour quota window and falls back to the first
  available ranked window.

## Agent and adapter behavior

- New data sources use the adapter contract. Source-specific parsing must not
  leak into storage or presentation contracts.
- Gemini CLI is explicitly outside the supported scope.
- Kimi Code and Kimi Work are one visible agent and one quota identity: show
  only `Kimi`. Their credential sources stay separate internally, while
  duplicate official windows keep the fresher reliable sample. The monthly OMNI
  cycle remains visible; gift and booster balances remain hidden.
- Do not expose credentials or raw provider responses through UI, logs, storage,
  sync, fixtures, or diagnostics.

## Platform forms

- The selected visual direction is `design/reference-option-2.png`. Metrik
  should feel restrained and platform-native through typography, material
  depth, spacing, and motion without imitating Apple branding or proprietary
  screens.
- The default desktop form is a compact approximately 380 × 440 widget. Full
  analytics are one click away in an expanded view. Pinning is opt-in.
- Platform-specific forms use Tauri's compile-time platform signal. A release
  must test that the native signal overrides a conflicting WebView user agent.

### Windows

- Compact transparency comes from a native whole-window system backdrop. Do not
  simulate glass by lowering only Metrik's own background opacity.
- Expanded mode remains opaque and owns its light/dark theme independently.
- Compact, strip, and expanded forms are reachable from one another in one
  click. Each form remembers its own position and never overwrites another
  form's position.
- Pinning and position lock apply only to compact and strip. Entering expanded
  mode always drops always-on-top and provides no pin control.
- Strip resizing preserves the screen edge it is flush to. Fully off-screen
  positions recover to the center and must never be persisted.
- Floating-form size uses the destination monitor's DPI. Compact and strip
  reassert size from native DPI-change payloads, and window mutations are
  serialized so stale resizes cannot overwrite corrections.
- The rendered CSS viewport is the final sizing authority for Windows floating
  forms. After native resize, compact and strip must compensate WebView zoom
  drift and verify the full design viewport rather than trusting HWND size alone.
- Strip window size is measured from rendered content. Constants may seed the
  first frame but are not the source of truth.
- Compact and strip have independent continuous UI scales in the range
  0.75–2.0, applied on the next entry into that form. Expanded mode is freely
  resizable and keeps webview zoom at 1.
- Border-drag scaling is intentionally unsupported without an OS-level hit-test
  solution and native regression coverage.

### macOS

- Compact mode is a native menu-bar panel that follows current system
  appearance and material. It is not a floating Windows-style strip.
- Content overlays must remain readable on both light and dark desktops.
- The menu bar uses Metrik's own minimal grammar: one monochrome provider icon
  plus official remaining percentage for every selected agent, `--` for
  unavailable data, and `~` for stale data.
- Clicking any status item opens the anchored compact panel. Agent selection
  updates immediately and always keeps at least one agent.
- Provider names are not repeated as menu-bar text, and the menu structure must
  not copy another product's layout or multi-account detail.

## Window state and statistics

- Pinning belongs only to floating forms. Expanded mode is always a normal
  window.
- On Windows, compact, strip, and expanded positions are remembered separately.
- Official quota failures, local parse coverage, and estimated pricing each
  retain their own status and provenance.
