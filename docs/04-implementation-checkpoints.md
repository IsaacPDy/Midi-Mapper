# 04 — Checkpoints and Next Tasks

## Checkpoint 0 — Verify starter on real macOS hardware

1. Install Rust, Node, Xcode Command Line Tools; run `npm install` and `npm run app:dev`.
2. Verify `midir` port identification and startup/Hotplug behavior with your actual controller.
3. Confirm Accessibility permission and `enigo` key injection across your common target apps.
4. Run test plan and fix compilation/hardware issues before expanding features.

**Acceptance:** your MIDI pad triggers a confirmed keyboard shortcut in a second app.

## Checkpoint 1 — Harden the recording experience

1. Add delay editing, press/release timing and an explicit 'clear draft' button.
2. Improve supported key coverage, physical-key identity vs. keyboard layout, and modifier-only behavior.
3. Add reliable error handling and an emergency 'stop replay' control.
4. Ensure recording state resets on editor close/minimize, focus loss, and window shutdown.

**Acceptance:** recording, cancel, and replay behave predictably across app switching.

## Checkpoint 2 — Mac-native menu bar and startup

1. Add Tauri tray icon; keep Rust service active with configuration window hidden.
2. Add launch-at-login, status indicator, reconnect notifications, and permission onboarding.
3. Add signing/notarization for distributing a real `.app`/`.dmg`.

**Acceptance:** mapped MIDI shortcuts work while the editor window is hidden.

## Checkpoint 3 — Expand beyond keyboard shortcuts

1. Relative encoder and absolute knob modes.
2. Continuous CC mappings for volume, scrolling, and app-specific sliders.
3. Per-app profiles and profile switching.
4. Reusable macros and optional hardware feedback.

**Acceptance:** keyboard mappings continue to work unchanged as new action types are introduced.
