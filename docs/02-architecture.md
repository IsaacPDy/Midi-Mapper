# 02 — Architecture and Data Model

```text
┌───────────────────┐
│ MIDI Controller   │
└────────┬──────────┘
         │ CoreMIDI / midir
         ▼
┌───────────────────┐       ┌──────────────────────┐
│ MIDI callback     ├──────►│ Bounded event queue  │
│ decode + try_send │       │ 128 event capacity   │
└───────────────────┘       └───────────┬──────────┘
                                      ▼
                            ┌──────────────────────┐
                            │ Rust action worker   │
                            │ CC rising-edge gate  │
                            │ mapping lookup       │
                            │ enigo replay         │
                            └─────────┬────────────┘
                                      │ event emission
                                      ▼
┌─────────────────────────────┐      ┌──────────────────┐
│ Rust state + mappings.json  │◄────►│ Tauri IPC        │
└─────────────────────────────┘      └────────┬─────────┘
                                              │
                                              ▼
                                     ┌──────────────────┐
                                     │ React UI         │
                                     │ MIDI Learn       │
                                     │ keyboard recorder│
                                     └──────────────────┘
```

## Ownership

- `model.rs`: Rust mapping and input types; MIDI 1.0 parser and tests.
- `engine.rs`: allowed key codes, validation, safe key press/release, sequential playback.
- `lib.rs`: IPC (`get_settings`, `list_devices`, `select_device`, `set_capture_active`, `save_mapping`, `delete_mapping`), config storage, connection monitor, worker, event emission.
- `App.tsx`: user-facing recording state machine; records `KeyboardEvent.code` and active modifiers, never stores drafts without confirm.
- `types.ts`: front-end equivalents of Rust DTOs.

## Recording state machine

`idle → learning → idle (selected control) → recording → idle (confirmed or canceled)`.

`set_capture_active(true)` pauses mapped playback while learning or recording; `false` restores normal MIDI action execution. Confirm performs persistence before leaving recording. No global event tap or separate macOS Input Monitoring entitlement is necessary for the first focused recorder.

## Mapping data example

```json
{
  "schema_version": 1,
  "selected_device_id": "coremidi-port-id",
  "mappings": [
    {
      "device_id": "coremidi-port-id",
      "control": { "kind": "note", "channel": 1, "number": 36 },
      "label": "Copy",
      "steps": [
        { "keys": ["Meta", "KeyC"], "delay_after_ms": 80 }
      ]
    }
  ]
}
```

Persist to Tauri's application config directory. The MIDI port ID originates with the MIDI backend; never invent it. JSON schema version 1 is the contract for the starter. Add migrations for future schema changes.

## Restrictions / improvements

1. MIDI device reconnection polls every 1.5 seconds; real device-change notifications and identity matching are future refinements.
2. One active MIDI port. Mappings are scoped by the selected port ID.
3. Worker executes one sequence at a time, preventing overlapping shortcut playback, but long sequences can delay later input events and the UI monitor.
4. Browser-based window recorder sees keydowns delivered to the WebView, not OS-reserved shortcuts. True global recording requires a separate consented macOS native event tap.
5. Using semantic Unicode keys in `enigo` can depend on active keyboard layout. Native virtual keycode translation can be added if hardware fidelity is a requirement.
6. KeyChord records *discrete presses*, not a perfectly timed key-down/key-up performance. Shift-only mappings and repeated hold behavior are out of scope.
7. Fixed CC threshold 64 treats some encoders incorrectly; these require new dedicated knob/encoder mappings later.
8. MIDI callbacks should remain nonblocking. `try_send` may discard messages if queue is full; instrument drop counters in production.
9. Control recording state lives in memory; the app remains usable without a cloud database.
10. Production should add key-stuck recovery, stronger macOS permission UX, a menu-bar/background process, and optional panic/stop-all-actions shortcut.
