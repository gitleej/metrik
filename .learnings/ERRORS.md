# Errors

Command failures and integration errors.

---

## [ERR-20260812-003] publisher_identity_test_matched_bundle_name

**Logged**: 2026-08-12T22:13:00+08:00
**Priority**: low
**Status**: resolved
**Area**: tests

### Summary
A new Widget publisher identity assertion matched `CFBundleName` as well as `CFBundleIdentifier`.

### Error
```
expected publisher Info.plist not to match /widget-publish<\/string>/
```

### Resolution
Scope the negative assertion to the value immediately following `CFBundleIdentifier`; the helper name remains intentionally `metrik-widget-publish`.

### Metadata
- Reproducible: yes
- Related Files: src/macosWidgetBundle.test.js

---

## [ERR-20260812-002] tauri_build_missing_cargo_path

**Logged**: 2026-08-12T22:08:00+08:00
**Priority**: low
**Status**: resolved
**Area**: build environment

### Summary
The repository's `npm run tauri build` could not find Cargo because the current shell PATH did not include the installed Rust toolchain.

### Error
```
failed to run command cargo metadata --no-deps --format-version 1: No such file or directory
```

### Resolution
Use the discovered toolchain bin directory `/Users/guoxiaoyu/.rustup/toolchains/1.88.0-aarch64-apple-darwin/bin` for this local build invocation. This is machine-specific and must not be added to repository configuration.

### Metadata
- Reproducible: yes
- Related Files: none

---

## [ERR-20260812-002] zsh_log_builtin_collision

**Logged**: 2026-08-12T21:55:00+08:00
**Priority**: low
**Status**: resolved
**Area**: tests

### Summary
The WidgetKit diagnostic invoked zsh's `log` builtin instead of macOS unified logging.

### Error
```
zsh:log:1: too many arguments
```

### Context
- Attempted to query recent WidgetKit and App Group events with `log show`.
- In zsh, the unqualified command resolved to a shell builtin.

### Suggested Fix
Invoke `/usr/bin/log show` explicitly in macOS diagnostics.

### Metadata
- Reproducible: yes
- Related Files: .learnings/ERRORS.md

---

## [ERR-20260812-001] local_tauri_updater_signing

**Logged**: 2026-08-12T21:45:00+08:00
**Priority**: low
**Status**: pending
**Area**: infra

### Summary
The local macOS Tauri bundle produced and signed `Metrik.app`, then returned an error because updater artifact signing has a public key configured but no local private key.

### Error
```
A public key has been found, but no private key. Set TAURI_SIGNING_PRIVATE_KEY.
```

### Context
- `npm run desktop:build -- --bundles app` completed the release binary, WidgetKit extension, app bundle, ad-hoc signing, and archive creation.
- The generated app passes `codesign --verify --deep --strict`.
- Release credentials are intentionally unavailable during local feature verification.

### Suggested Fix
Treat the verified `.app` as the local native smoke artifact. Keep updater signing in the serialized release workflow rather than adding release credentials to local feature work.

### Metadata
- Reproducible: yes
- Related Files: src-tauri/tauri.conf.json

---

## [ERR-20260811-002] macos_widget_gallery_desktop_drop

**Logged**: 2026-08-11T19:56:00+08:00
**Priority**: medium
**Status**: pending
**Area**: tests

### Summary
The macOS widget gallery exposed Metrik's preview and the completed local install, but Computer Use could not complete the final drag from the gallery into the Finder desktop surface.

### Error
```
Computer Use server error -10005: noWindowsAvailable
Computer Use server error -10005: windowNotFoundAtPosition
```

### Context
- Rebuilt and installed `/Applications/Metrik Widget Preview.app`.
- Notification Center's widget gallery listed all four Metrik family previews, including the large approved card.
- The gallery accessibility tree describes the desktop target as “将小组件拖放到桌面…”, but it exposes no actionable desktop drop element. Finder's Desktop accessibility container likewise has no window-backed coordinate target.

### Suggested Fix
Leave the gallery open and ask the user to drag the large Metrik preview onto the desktop once; keep the installed extension and WidgetKit Simulator as automated verification surfaces.

### Metadata
- Reproducible: yes
- Related Files: scripts/build-macos-widget-preview.sh, WidgetExtension/Sources/MetrikWidgetBundle.swift
- See Also: ERR-20260809-002

## [ERR-20260811-001] temporary_binary_inspection

**Logged**: 2026-08-11T09:53:00+08:00
**Priority**: low
**Status**: resolved
**Area**: tests

### Summary
A Mach-O inspection command was rejected because its cleanup step used a recursively destructive command on a temporary directory.

### Error
```
Rejected: rm -f style commands are not permitted. Use a safer approach.
```

### Context
- The command thinned a third-party universal widget binary into a fresh `mktemp` directory for entry-point inspection.
- The diagnostic itself was read-only; only the cleanup form triggered the safety rejection.

### Resolution
Repeated the inspection without deleting the temporary directory and confirmed that a valid WidgetKit extension's `LC_MAIN` points to the `_NSExtensionMain` symbol stub.

### Metadata
- Reproducible: yes
- Related Files: scripts/build-macos-widget-preview.sh

---

## [ERR-20260810-002] direct_app_group_directory_creation

**Logged**: 2026-08-10T22:18:00+08:00
**Priority**: high
**Status**: in_progress
**Area**: macOS shell

### Summary
Creating an App Group directory by spelling its `~/Library/Group Containers` path blocked in macOS ContainerManager.

### Error
```
The publisher remained blocked in mkdir for the group.app.metrik.desktop directory.
```

### Resolution
Moved App Group resolution into a small signed Swift helper that calls `FileManager.containerURL(forSecurityApplicationGroupIdentifier:)`. The helper correctly obtained the container, but file creation remained blocked while the Mac was locked. Local packaging no longer waits on data publication; the extension uses bundled preview data until the unlocked host publishes its first snapshot. Unbundled Rust builds write only to an explicit Application Support preview fallback.

### Metadata
- Reproducible: yes
- Related Files: src-tauri/src/widget_snapshot.rs, WidgetExtension/Sources/MetrikWidgetPublisher.swift

---

## [ERR-20260810-001] widget_snapshot_open_options_trait

**Logged**: 2026-08-10T22:03:00+08:00
**Priority**: low
**Status**: resolved
**Area**: backend

### Summary
The first macOS widget snapshot compile omitted the Unix `OpenOptionsExt` trait import.

### Error
```
no method named `mode` found for struct `OpenOptions`
```

### Resolution
Imported `std::os::unix::fs::OpenOptionsExt` alongside `PermissionsExt`; the file mode remains explicitly restricted to the current user.

### Metadata
- Reproducible: yes
- Related Files: src-tauri/src/widget_snapshot.rs

---

## [ERR-20260809-006] local_skill_and_patch_context

**Logged**: 2026-08-09T22:30:39+08:00
**Priority**: low
**Status**: resolved
**Area**: workflow

### Summary
The initial self-improvement skill path and the first combined CSS patch context were incorrect.

### Error
```
sed: .../.codex/skills/self-improving-agent/SKILL.md: No such file or directory
apply_patch verification failed: Failed to find expected lines in src/styles.css
```

### Resolution
Used the available-skills root mapping for the skill and split the patch into smaller exact-context edits.

### Metadata
- Reproducible: yes
- Related Files: src/styles.css, .learnings/LEARNINGS.md

---

## [ERR-20260809-001] verification_toolchain

**Logged**: 2026-08-09T19:41:59+08:00
**Priority**: medium
**Status**: resolved
**Area**: infra

### Summary
The repository verification baseline initially could not start because frontend dependencies and Rust shims were absent from the active shell path.

### Error
```
vite: command not found
cargo: command not found
```

### Context
- `npm test` passed because those tests use Node directly.
- `npm run build` requires the local Vite dependency.
- `cargo fmt --check` requires an installed Rust toolchain.

### Resolution
Installed locked frontend dependencies with `npm ci`. The Rust toolchain was present under the user's Cargo directory, so the checks were repeated with that directory prepended to `PATH`. Frontend build, Rust formatting, Clippy, and Rust tests then passed.

### Metadata
- Reproducible: yes
- Related Files: package-lock.json, src-tauri/Cargo.toml

---

## [ERR-20260809-002] computer_use_native_smoke

**Logged**: 2026-08-09T19:52:25+08:00
**Priority**: medium
**Status**: pending
**Area**: tests

### Summary
Computer Use could not attach to the launched Metrik macOS app for an automated native visual smoke check.

### Error
```
timeoutReached
```

### Context
- The Tauri development command compiled and launched `target/debug/metrik` successfully.
- App-state probes by app name and bundle identifier timed out, so native z-order, Spaces, and drag-position behavior could not be visually inspected.
- Browser-rendered visual and interaction QA remained available and passed.

### Suggested Fix
Repeat the smoke check in an interactive macOS session where Screen Recording and Accessibility access are available to the automation host.

### Metadata
- Reproducible: yes
- Related Files: src-tauri/src/macos.rs, design-qa.md

### Recurrence
- **Last Seen**: 2026-08-12T21:52:00+08:00
- **Recurrence Count**: 6
- **Notes**: The installed WidgetKit preview and WidgetKit Simulator were discoverable and native rendering succeeded, but macOS locked again before the automated desktop-gallery placement pass. A direct Dock attachment also timed out. On the latest run CoreGraphics could enumerate Notification Center's desktop-widget windows, but `screencapture -l` could not image those system-owned negative-layer windows; System Events then rejected menu-bar access because `osascript` lacks Accessibility permission. System compilation, registration, snapshot, and signing checks remained available.

---

## [ERR-20260809-004] tauri_dev_rust_path

**Logged**: 2026-08-09T20:31:00+08:00
**Priority**: low
**Status**: resolved
**Area**: infra

### Summary
The first native launch used the wrong Cargo shim path.

### Error
```
failed to run command cargo metadata: No such file or directory
```

### Context
- Cargo is installed in the active Rust toolchain directory, not `/Users/guoxiaoyu/.cargo/bin`.

### Resolution
Launched the Tauri command with the concrete Rust toolchain `bin` directory in `PATH`; the app compiled successfully.

### Metadata
- Reproducible: yes
- Related Files: src-tauri/Cargo.toml

### Recurrence
- **Last Seen**: 2026-08-12T21:40:00+08:00
- **Recurrence Count**: 3
- **Notes**: The continued WidgetKit verification shell again omitted the Rust toolchain from PATH. Calling the `cargo` binary by absolute path was insufficient because Cargo also resolves `cargo-fmt` and `cargo-clippy` through PATH; prepend the whole toolchain `bin` directory. A later overly narrow PATH also hid Homebrew's `npm`, so native builds need both toolchain directories. These are environment failures, not source failures.

---

## [ERR-20260809-003] in_app_browser_api_assumption

**Logged**: 2026-08-09T19:52:25+08:00
**Priority**: low
**Status**: resolved
**Area**: tests

### Summary
Initial browser QA calls assumed Playwright page methods that are not exposed by the in-app Browser wrapper.

### Error
```
setViewportSize is not a function
boundingBox is not a function
getConsoleLogs is not a function
```

### Context
- The first attempts used direct Playwright-style page and locator APIs.
- The Browser wrapper exposes viewport control through `viewport.set`, screenshots through the tab, and logs through `tab.dev.logs`.

### Resolution
Used the wrapper-specific APIs and completed the settings-toggle, full-view, screenshot, and console-log checks.

### Metadata
- Reproducible: yes
- Related Files: design-qa.md

### Recurrence
- **Last Seen**: 2026-08-09T22:30:39+08:00
- **Recurrence Count**: 5
- **Notes**: The prior deliverable tab was no longer part of the session, so QA created a fresh tab from the existing Browser binding. Page evaluation is read-only and rejected a temporary `textContent` mutation; exact layout verification used the real demo value plus tabular-numeral geometry instead.

---

## [ERR-20260809-005] product_design_reference_path

**Logged**: 2026-08-09T20:58:47+08:00
**Priority**: low
**Status**: resolved
**Area**: workflow

### Summary
The first critical-overrides read used the skill directory instead of the package-level references directory.

### Error
```
sed: .../skills/critical-overrides.md: No such file or directory
```

### Resolution
Resolved the relative path from `design-qa/SKILL.md` and read `../../references/critical-overrides.md` completely before continuing.

### Metadata
- Reproducible: yes
- Related Files: design-qa.md

---

## [ERR-20260812-001] openai_docs_search_auth

**Logged**: 2026-08-12T17:13:55+08:00
**Priority**: low
**Status**: pending
**Area**: docs

### Summary
Official OpenAI documentation search failed because the web-search authentication token had been invalidated.

### Error
```
401 Unauthorized: token_invalidated
```

### Context
- Attempted a read-only official documentation search for Codex configuration, skills, MCP performance, session cleanup, and troubleshooting.
- No credentials or token values were recorded.
- Local read-only diagnostics remained available.

### Suggested Fix
Refresh the Codex/OpenAI sign-in session before the next documentation search, then retry against official OpenAI domains.

### Metadata
- Reproducible: unknown
- Related Files: none

---
---

## [ERR-20260812-001] macos_widget_sandbox_and_deeplink_deadlock

**Logged**: 2026-08-12T18:30:00+08:00
**Priority**: high
**Status**: resolved
**Area**: macOS shell

### Summary
Two hard-won platform facts from the 0.15.2 widget work: (1) macOS app extensions MUST be sandboxed — pkd rejects unsandboxed appex with "plug-ins must be sandboxed", and ad-hoc signed code gets no temporary-exception file access, so an unsandboxed widget reading Application Support is impossible; (2) the 0.15.1 "wants to access data from other apps" prompt storm came from the publisher helper touching the App Group container WITHOUT app-sandbox — sandboxed processes carrying the group entitlement access the container with no TCC prompt (verified on macOS 26.5). Also: creating a Tauri window synchronously inside a deep-link URL event listener (AppleEvent handler) self-deadlocks the main thread on a mutex — defer with run_on_main_thread if deep links ever return.

### Error
```
pkd: rejecting; Ignoring mis-configured plugin at [MetrikWidget.appex]: plug-ins must be sandboxed
sample: AEProcessAppleEvent → _handleAEGetURLEvent → open_expanded_window → __psynch_mutexwait (main-thread deadlock)
```

### Resolution
Keep the App Group bridge; sign metrik-widget-publish with BOTH app-sandbox and application-groups (WidgetExtension/MetrikWidgetPublisher.entitlements); host app holds no group entitlement and only pipes JSON to the helper via stdin. Widget click-through (metrik:// deep links) was reverted entirely in fd0e9bb — widgets are display-only.

### Metadata
- Reproducible: yes
- Related Files: scripts/build-macos-widget-extension.sh, WidgetExtension/MetrikWidgetPublisher.entitlements, src-tauri/src/widget_snapshot.rs

---
