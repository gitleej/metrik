# Learnings

Corrections, insights, and knowledge gaps captured during development.

**Categories**: correction | insight | knowledge_gap | best_practice

---

## [LRN-20260812-001] correction

**Logged**: 2026-08-12T21:55:00+08:00
**Priority**: high
**Status**: resolved
**Area**: macOS shell

### Summary
An App Group snapshot containing four selected Agents does not prove the desktop widget is rendering that snapshot.

### Details
The user screenshot still showed the six-Agent bundled preview while the shared JSON correctly contained four Agents. The rendered values matched `MetrikWidgetStore.preview`, indicating a stale or fallback WidgetKit extension. Local extension builds reused `CFBundleVersion=1`, so repeated registration could retain cached extension code.

### Suggested Action
Give every local WidgetKit build a monotonically changing bundle build number, then re-register and verify the rendered card rather than inferring UI state from the shared JSON alone.

### Metadata
- Source: user_feedback
- Related Files: scripts/build-macos-widget-extension.sh, WidgetExtension/Sources/MetrikWidgetModels.swift
- Tags: widgetkit, cache, native-qa

### Resolution
- **Resolved**: 2026-08-13T12:46:35+08:00
- **Notes**: The snapshot publisher and extension were both correct. WidgetKit generated the new timeline but chronod rejected it because the extension alone used a timestamp `CFBundleVersion` that differed from the containing app. Production builds now give the containing app and extension the same build and release versions, and release CI verifies the final bundle. A production-form local install then decoded the live three-Agent snapshot, returned `Reload success` for both widget kinds, updated all four timeline files, and visibly changed the desktop card from 241.7M / GLM 96% to 31.2M / GLM 93%.

---

## [LRN-20260811-002] correction

**Logged**: 2026-08-11T10:32:00+08:00
**Priority**: critical
**Status**: resolved
**Area**: product design

### Summary
Migrating an accepted desktop-card design to WidgetKit changes the platform shell, not the approved information architecture or visual identity.

### Details
The first native large widget replaced the accepted quota-dial, today-summary, Agent-switcher and footer composition with a generic six-row list. That solved native corners and material but constituted an unrequested redesign. The user's reference made clear that the original card was already acceptable; only text/ring positioning, native transparency, and multi-Agent completeness needed correction.

### Suggested Action
When replacing simulated system chrome with a native extension, treat the approved screenshot as the sole layout source of truth. Preserve its anatomy and typography, assign only the outer material/corners/margins to WidgetKit, and adapt the existing Agent region surgically when more items must fit.

### Metadata
- Source: user_feedback
- Related Files: WidgetExtension/Sources/MetrikWidgetViews.swift, docs/PRODUCT-CONSTRAINTS.md
- Tags: widgetkit, visual-fidelity, scope-control, native-migration
- Pattern-Key: preserve_accepted_anatomy_when_replacing_platform_shell
- Recurrence-Count: 1
- First-Seen: 2026-08-11
- Last-Seen: 2026-08-11

### Resolution
- **Resolved**: 2026-08-11T10:32:00+08:00
- **Notes**: Restored the quota dial, today token summary, Agent switcher, and footer inside the native WidgetKit container. Lowered the dial copy, reduced the serif percentage reading and ring stroke, and adapted the original switcher to a two-column six-Agent grid without overflow.

---

## [LRN-20260810-003] correction

**Logged**: 2026-08-10T20:20:00+08:00
**Priority**: critical
**Status**: resolved
**Area**: architecture

### Summary
A borderless Tauri WebView with `NSVisualEffectView` is not a faithful substitute for a macOS WidgetKit desktop widget, and material opacity is not a CSS design token.

### Details
Repeated CSS offset, scrim, radius, and percentage-size changes did not fix the native screenshots. The title still overlaps the painted quota arc, and moving the density slider produces little perceived change because the semantic AppKit material remains the dominant surface. Research of CodexBar shows that its desktop widgets are a real WidgetKit app extension using the system widget container background, while its menu surface is an AppKit `NSMenu`; neither surface is implemented as a WebView pretending to be system chrome.

### Suggested Action
Stop tuning the current simulated desktop window. Re-evaluate the macOS shell as three explicit surfaces: native AppKit menu, real WidgetKit extension, and the existing Tauri analytics window. Put widget content on system margins, keep provider/window text outside the gauge stroke envelope, share sanitized data through an App Group snapshot, and make signing plus native WidgetKit installation part of local acceptance before release.

### Metadata
- Source: user_feedback
- Related Files: src-tauri/src/macos.rs, src/App.jsx, src/styles.css, docs/PRODUCT-CONSTRAINTS.md
- Tags: macos, widgetkit, appkit, nsvisualeffectview, tauri, native-shell
- See Also: LRN-20260809-003, LRN-20260809-004
- Pattern-Key: research_native_surface_before_simulating_system_chrome
- Recurrence-Count: 1
- First-Seen: 2026-08-10
- Last-Seen: 2026-08-10

### Resolution
- **Resolved**: 2026-08-11T10:20:00+08:00
- **Notes**: Replaced the simulated desktop WebView with a real WidgetKit extension, moved the widget surface to the system container background, published a sanitized App Group snapshot contract, and installed a locally registered preview host. Native WidgetKit Simulator checks passed for small, medium, and large families; the large family rendered all six supported Agents without clipping or painted-ring overlap.

---

## [LRN-20260811-001] best_practice

**Logged**: 2026-08-11T10:20:00+08:00
**Priority**: high
**Status**: resolved
**Area**: macOS shell

### Summary
A directly linked local WidgetKit preview extension must enter through `_NSExtensionMain`, use a valid changing `CFBundleVersion`, and be indexed only after the installed host is fully copied.

### Details
The first ad-hoc extension compiled and signed but WidgetKit listed an empty descriptor because its Mach-O `LC_MAIN` still pointed at Swift's ordinary `_main`; the process launched and exited voluntarily. Comparing a known working native widget showed that the entry point must resolve to `_NSExtensionMain`. WidgetKit and Launch Services also retained stale extension metadata when the local build number did not change or registration ran immediately after replacing the host.

### Suggested Action
For non-Xcode local WidgetKit previews, link with `_NSExtensionMain`, sandbox both host and extension, generate a valid monotonically changing preview build number, wait briefly after installation, then re-register and verify the extension with `pluginkit` plus a native Simulator render.

### Metadata
- Source: debugging
- Related Files: scripts/build-macos-widget-preview.sh, WidgetExtension/Sources/MetrikWidgetBundle.swift
- Tags: macos, widgetkit, nsextensionmain, launch-services, preview
- Pattern-Key: harden.direct_widgetkit_preview_packaging
- Recurrence-Count: 1
- First-Seen: 2026-08-11
- Last-Seen: 2026-08-11

### Resolution
- **Resolved**: 2026-08-11T10:20:00+08:00
- **Notes**: The preview build now links through `_NSExtensionMain`, changes a valid local build number, waits before `pluginkit` registration, passes deep code-sign verification, and renders in WidgetKit Simulator.

---

## [LRN-20260810-002] correction

**Logged**: 2026-08-10T16:33:45+08:00
**Priority**: critical
**Status**: resolved
**Area**: frontend

### Summary
A single quota headline must show the window that currently constrains the Agent, not blindly the first or shortest window.

### Details
The macOS feature branch predated the shared `bindingWindow` logic on the latest main branch. Its desktop widget and status item always preferred the first live window. That can report a comfortable five-hour remainder while the weekly window is nearly exhausted, so the displayed number does not answer whether the Agent can actually be used. The official Codex App Server conversion itself was correct: on this Mac it returned a weekly window with 9% used, which Metrik correctly stored as 91% remaining.

### Suggested Action
Use one tested shared selector across compact rows, strip cells, menu-bar status items, and the macOS desktop dial. Prefer the shortest live window normally; once any window reaches 15% remaining, display the lowest live window. Always show the selected window label (`5h`, `7d`, or other cycle) next to the prominent Agent name.

### Metadata
- Source: user_feedback
- Related Files: src/quotaWindows.js, src/quotaWindows.test.js, src/App.jsx, docs/PRODUCT-CONSTRAINTS.md
- Tags: quota, codex, binding-window, macos, shared-logic
- See Also: LRN-20260810-001
- Pattern-Key: harden.quota_headline_uses_binding_window
- Recurrence-Count: 1
- First-Seen: 2026-08-10
- Last-Seen: 2026-08-10

### Resolution
- **Resolved**: 2026-08-10T16:33:45+08:00
- **Notes**: Backported the latest main-branch binding-window selector, applied it to all one-number quota surfaces including the macOS desktop dial, added the visible window label, and added eleven regression tests.

---

## [LRN-20260810-001] correction

**Logged**: 2026-08-10T15:13:29+08:00
**Priority**: high
**Status**: resolved
**Area**: frontend

### Summary
Do not infer the repository's latest platform behavior from a clone whose fetch refspec tracks only the current feature branch.

### Details
The local clone initially made the Windows settings appear to retain an old 60% transparency floor. An explicit remote-ref inspection showed that current Windows work already uses true-alpha composition and a 5–96% density range. Reusing one density value for the macOS menu-bar panel and desktop widget would still couple two different native materials and prevent a genuinely clear desktop surface.

### Suggested Action
When cross-platform behavior is cited as precedent, inspect `git remote` refspecs and explicit remote heads before comparing implementations. Keep macOS panel density (5–96%) and desktop-widget background density (0–96%) as separate persisted settings; define 0% as no CSS tint above native `UnderWindowBackground` vibrancy.

### Metadata
- Source: user_feedback
- Related Files: src/App.jsx, src/styles.css, docs/PRODUCT-CONSTRAINTS.md
- Tags: git-refspec, windows, macos, transparency, independent-settings
- See Also: LRN-20260809-004
- Pattern-Key: verify.remote_platform_precedent_and_decouple_material_controls
- Recurrence-Count: 1
- First-Seen: 2026-08-10
- Last-Seen: 2026-08-10

### Resolution
- **Resolved**: 2026-08-10T15:13:29+08:00
- **Notes**: Fetched the explicit remote Windows refs, backported the 5% panel floor, and added an independent 0–96% desktop-widget background control with a true no-tint minimum.

---

## [LRN-20260809-004] correction

**Logged**: 2026-08-09T22:30:39+08:00
**Priority**: high
**Status**: resolved
**Area**: frontend

### Summary
For a compact quota dial, users judge `95%` as one visual object, and HUD vibrancy is not equivalent to the much clearer macOS desktop-widget surface.

### Details
Centering only the primary digits while hanging the percent glyph produced a correct numeral center but a persistently right-heavy complete reading. The minimum CSS tint was already only about six percent in light mode, so the remaining milky opacity came primarily from `NSVisualEffectMaterialHUDWindow`, not the slider mapping.

### Suggested Action
Center and size the complete percentage group, verify its bounding-box center against the ring, and use `UnderWindowBackground` for the optional desktop widget while retaining `HudWindow` for the menu-bar panel. At the minimum density, remove the CSS tint entirely.

### Metadata
- Source: user_feedback
- Related Files: src/App.jsx, src/styles.css, src/windowClient.js, docs/PRODUCT-CONSTRAINTS.md
- Tags: macos, vibrancy, under-window-background, optical-alignment, typography
- See Also: LRN-20260809-003
- Pattern-Key: harden.native_widget_material_and_group_alignment
- Recurrence-Count: 1
- First-Seen: 2026-08-09
- Last-Seen: 2026-08-09

### Resolution
- **Resolved**: 2026-08-11T10:32:00+08:00
- **Notes**: Retired the WebView/AppKit desktop-window substitute. The approved focus-dial anatomy now renders inside a real WidgetKit large family with a single centered serif percentage reading, lowered copy, safe ring geometry, and system-owned material.

---

## [LRN-20260809-003] correction

**Logged**: 2026-08-09T20:42:17+08:00
**Priority**: high
**Status**: resolved
**Area**: frontend

### Summary
Browser fidelity cannot validate native material, NSWindow corners, or cross-window settings propagation.

### Details
The packaged macOS screenshots exposed three issues the browser pass did not: a borderless window shadow produced an outer rectangular corner, the CSS tint visually dominated native vibrancy, and Agent selections made in the expanded WKWebView were not live in the desktop-widget WKWebView. The percentage group was geometrically centered, but the primary number itself still looked offset because the percent glyph participated in centering.

### Suggested Action
For macOS shell changes, make the packaged `.app` the release gate. Treat suffixes such as `%` as optical hangers, turn off rectangular native shadows for rounded transparent windows, keep the CSS scrim thin above native vibrancy, and broadcast shared UI settings across WKWebViews.

### Metadata
- Source: user_feedback
- Related Files: src/App.jsx, src/styles.css, src/windowClient.js, src-tauri/src/macos.rs
- Tags: macos, vibrancy, nswindow, optical-alignment, cross-window-state
- See Also: LRN-20260809-001, LRN-20260809-002
- Pattern-Key: harden.native_shell_visual_gate
- Recurrence-Count: 2
- First-Seen: 2026-08-09
- Last-Seen: 2026-08-09

### Resolution
- **Resolved**: 2026-08-09T20:49:00+08:00
- **Notes**: Centered the main number independently, reduced the native-mode scrim, disabled the rectangular NSWindow shadow, broadcast Agent selections, and rendered all selected Agents adaptively.

---

## [LRN-20260809-002] correction

**Logged**: 2026-08-09T20:24:00+08:00
**Priority**: high
**Status**: resolved
**Area**: frontend

### Summary
Geometric centering is insufficient when text still crosses a progress ring's painted safe area.

### Details
The percentage group was numerically centered after the first correction, but the top Agent label still touched the blue arc and the full-width reset line crossed the lower-left arc. The final check must include the painted stroke envelope, not only element center coordinates.

### Suggested Action
Define an explicit inner safe area for text inside circular visualizations. Keep labels away from the stroke, constrain secondary copy to a narrower measure, and verify the result in the native Retina WebView before release.

### Metadata
- Source: user_feedback
- Related Files: src/styles.css, src-tauri/src/macos.rs
- Tags: progress-ring, optical-alignment, safe-area, native-smoke
- See Also: LRN-20260809-001
- Pattern-Key: harden.circular_visual_safe_area
- Recurrence-Count: 1
- First-Seen: 2026-08-09
- Last-Seen: 2026-08-09

### Resolution
- **Resolved**: 2026-08-09T20:29:00+08:00
- **Notes**: Moved the copy block into the ring, reduced the display value slightly, and narrowed the reset line from 118px to 88px.

---

## [LRN-20260809-001] correction

**Logged**: 2026-08-09T20:08:34+08:00
**Priority**: medium
**Status**: resolved
**Area**: frontend

### Summary
Keep the quota-ring copy on an explicit track that shares the SVG ring's center.

### Details
The copy container declared a `90px` width, but its reset label created a `118px` implicit CSS Grid column. The column started at the container's left edge, shifting the Agent label and `72%` group 14px to the right of the ring center even though the container itself appeared centered.

### Suggested Action
Give overlay grids an explicit `minmax(0, 1fr)` column and align their full width to the geometry they annotate; verify the element centers numerically as well as visually.

### Metadata
- Source: user_feedback
- Related Files: src/styles.css
- Tags: css-grid, alignment, quota-ring, visual-qa
- Pattern-Key: harden.overlay_alignment
- Recurrence-Count: 1
- First-Seen: 2026-08-09
- Last-Seen: 2026-08-09

### Resolution
- **Resolved**: 2026-08-09T20:12:00+08:00
- **Notes**: Aligned the copy overlay to the ring's `128px` SVG box and constrained its grid track explicitly.

---
