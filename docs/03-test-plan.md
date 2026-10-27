# 03 — Hardware Verification and Acceptance Tests

Run these on an actual Mac with at least one supported CoreMIDI input device. A non-Mac static build does not replace these tests.

## First-run checks

- [ ] `npm install` completes; `npm run build` finishes with no TypeScript errors.
- [ ] `cargo test` under `src-tauri` succeeds on macOS.
- [ ] `npm run app:dev` launches the Tauri window.
- [ ] MIDI device appears in dropdown and status changes to Device available.
- [ ] When MIDI device is unplugged/reconnected, the app remains responsive and eventually reconnects.

## Recording and playback

- [ ] Click Learn, press Note On pad: channel and note number are correct.
- [ ] Press Note Off (or Note On velocity 0): does not replay.
- [ ] Record `⌘+C`, confirm, focus another app, press pad: selected text is copied.
- [ ] Record `⌘+C`, `⌘+V`: displayed order and playback order match.
- [ ] Record and click Undo last: final step is removed.
- [ ] Record and click Cancel: original mapping remains untouched.
- [ ] Close/reopen app: confirmed mapping persists.
- [ ] Delete mapping: no action fires for that control.
- [ ] While recording is active, pressing a previously mapped MIDI pad does not execute it.
- [ ] CC button with values 0 → 127 → 127 → 0 → 127 triggers only twice.

## Permissions and failure modes

- [ ] Deny Accessibility permission: playback error appears (not silent failure).
- [ ] Grant Accessibility permission: playback works after any required app relaunch.
- [ ] Unplug controller during capture: UI still allows cancellation.
- [ ] Rapidly tap pad several times: app does not crash or leave modifiers held.
- [ ] Press an unsupported key code: it is not saved.
- [ ] Test both US and non-US keyboard layouts if relevant; record mismatches.
- [ ] Test multiple identical controller names: selection resolves correct physical port.

## Mac-specific warning

A focused WebView **cannot reliably capture shortcuts reserved by macOS**, especially app/window switching. If the project needs `⌘+Tab` recording, build a separate, explicit-consent native capture mode and validate the associated permissions. Do not silently install global keyboard monitoring.

## Definition of done for MVP

MIDI device is visible, MIDI Learn chooses the intended pad, confirming `⌘+C` persists the mapping, pressing the pad copies in a different foreground app, and a canceled draft changes nothing.
