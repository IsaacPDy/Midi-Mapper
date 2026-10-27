# 05 — Product Vision and Feature Roadmap

This document imagines MIDI Mapper after a successful public launch, then works backwards into an ordered plan. Checkpoints 0–3 in `04-implementation-checkpoints.md` are folded into Phases 1–3 below.

## North star

> Plug in any MIDI controller. MIDI Mapper recognises it, draws it on screen, and suggests a starter layout for the app you're in. The pads light up to show what they do. Switch to DaVinci Resolve and the whole surface changes; switch to OBS and the live scene's pad glows. Press one pad and the mic mutes, and that pad stays red until the mic is actually unmuted. It runs in the menu bar, works offline, and never types anything you didn't ask it to.

The pitch in one line: **your $60 MIDI controller becomes a Stream Deck with real knobs and faders, wired into the apps you already use.**

### Who it's for

| User | Typical controller | What "works wonders" means for them |
|---|---|---|
| Video editor | Jog wheel, nanoKONTROL | Scrub with a knob, mark in/out on pads, colour faders that pick up where the value already is |
| Streamer / podcaster | Launchpad, APC mini | Scene pads that light the live scene; a mute pad that always matches the real mic state |
| Photographer | X-Touch Mini | Exposure, temperature and highlights on faders in Lightroom; pick/reject on pads |
| Musician | The keyboard they already own | One controller drives both the DAW and the desktop, using a shift layer |
| Presenter / teacher | Small pad controller | Slides, timer, mute and screen switching without hunting for the keyboard |
| Accessibility user | Large pads, foot pedals | One press for any complex shortcut, big on-screen labels, no fine-motor chords |
| Developer / power user | Anything | Pads that trigger Shortcuts, Keyboard Maestro macros, editor commands, home lights |

---

## How it all connects

Every feature in this plan plugs into one pipeline. The current MVP already has a small version of it: decode → find mapping → replay keys. The finished product widens each stage and adds two new pieces, a **state store** and a **feedback engine**.

```text
 INPUTS                       CORE PIPELINE (Rust service in the menu-bar agent)           OUTPUTS

 MIDI 1.0 / 2.0 devices ─┐                                                            ┌─► Keyboard & mouse
 Virtual MIDI ports ─────┤   ┌───────────┐   ┌───────────┐   ┌──────────┐   ┌───────┐ ├─► System: volume, media, mic, Focus
 iPad / network MIDI ────┼──►│1 Normalize├──►│2 Gesture  ├──►│3 Route   ├──►│4 Act  ├─┼─► App integrations (OBS, Lightroom…)
 Local API / MCP ────────┘   │ identity, │   │ tap, hold,│   │ profile, │   │ queues│ ├─► MIDI / OSC / HTTP out
                             │ encoders, │   │ double,   │   │ layer,   │   │ panic │ └─► Shortcuts, Keyboard Maestro…
                             │ 14-bit    │   │ chord     │   │ "when"   │   └───┬───┘
                             └───────────┘   └───────────┘   └────▲─────┘       │
                                                                  │ reads       │ writes toggles, variables
   Front app, Focus mode, time ──────────────►┌───────────────────┴─────────────▼───┐
                                              │ State store                         │◄── Integration events
                                              │ toggles · variables · active profile│    (scene changed, mic
                                              │ · app and integration state         │     muted, light on…)
                                              └──────────────────┬──────────────────┘
                                                                 ▼
                                                        ┌─────────────────┐
                                                        │5 Feedback       ├──► pad LEDs · motor faders ·
                                                        └─────────────────┘    scribble strips · HUD · menu bar
```

| Stage | Today (v0.1) | Finished product |
|---|---|---|
| 1 Normalize | Note On/Off and CC ≥ 64 from one port (`model.rs`) | Many devices with stable identity; absolute, relative-encoder, 14-bit, NRPN, pitch bend, aftertouch; MIDI 2.0 |
| 2 Gesture | Press only (CC rising edge in `lib.rs`) | Press, release, tap, double-tap, long-press, hold-to-repeat, two-pad chords, toggle |
| 3 Route | Exact match on device + control | Profile (per app) → layer (shift/pages) → conditions ("when OBS is recording") |
| 4 Act | Key chords, fixed 80 ms delay, one at a time (`engine.rs`) | Typed action chains with per-binding queue policy, timing editor, panic stop |
| State store | None | Shared source of truth read by routing and feedback, written by actions and integrations |
| 5 Feedback | Last MIDI event in the UI | LED colours, motor faders, displays, on-screen HUD, menu-bar icon |

### The one idea that ties integrations together

Every integration, built-in or third-party, offers the same three things:

- **Actions**: what a pad can make it do (`obs.switch_scene`, `hue.set_brightness`).
- **State**: values it publishes to the state store (`obs.current_scene`, `obs.input_muted:Mic`).
- **Events**: changes that can start an automation ("when OBS starts recording, switch to the Live layer").

Actions are outbound, events are inbound, and state powers both the LEDs and the "when" conditions. With that contract in place, adding Spotify, Home Assistant or a community plugin is the same job every time.

### Data model v2 (sketch)

Today a mapping is `device + control + label + steps: KeyChord[]` (`model.rs:36`). Version 2 generalises it without losing anything:

```json
{
  "schema_version": 2,
  "devices": [
    { "id": "lp1", "match": { "name": "Launchpad Mini MK3", "port_hint": "coremidi-port-id" },
      "template": "novation/launchpad-mini-mk3" }
  ],
  "profiles": [
    {
      "id": "streaming", "name": "Streaming",
      "activate_when": { "front_app": ["com.obsproject.obs-studio"] },
      "layers": [{
        "id": "base",
        "bindings": [{
          "device": "lp1",
          "control": { "kind": "note", "channel": 1, "number": 36, "behavior": "button" },
          "trigger": { "gesture": "tap" },
          "when": [{ "state": "obs.connected", "equals": true }],
          "actions": [{ "type": "obs.toggle_input_mute", "input": "Mic" }],
          "feedback": { "state": "obs.input_muted:Mic", "on": "red", "off": "green" },
          "queue": "drop_if_busy"
        }]
      }]
    }
  ]
}
```

**Migration v1 → v2:** each v1 mapping becomes a binding in a default "Everywhere" profile with `actions: [{ "type": "keys", "steps": [...] }]`. It is lossless and runs automatically on first launch, with a backup of the old file.

---

## Feature catalog (the finished product)

### Inputs
- Multiple controllers at once, with stable identity across reboots and USB ports (match on name, port, and MIDI-CI identity where available).
- **Device templates**: a library of known controllers with layout, LED protocol and encoder mode, so the editor draws the actual hardware and you click a pad on screen to edit it.
- Encoder mode auto-detect ("turn the knob left, then right").
- Virtual MIDI input, so any app can trigger mappings, and an iPad/iPhone companion that works as a touch controller.
- MIDI 2.0 / UMP and MIDI-CI discovery, so a device can name itself and auto-load its template.

### Triggers and gestures
- Tap, double-tap, long-press, hold-to-repeat, separate press and release actions, toggle on/off.
- Two-pad chords, and velocity zones (soft hit = one action, hard hit = another).
- **Shift layers**: hold a pad to change what every other pad does; page buttons for banks.
- Automations: state-change triggers that need no physical input ("when Zoom call starts → Meeting layer").

### Actions
| Group | Actions |
|---|---|
| Keyboard | Chords and sequences (today), typed text snippets, hold durations, per-step delays, repeat-while-held |
| Mouse | Click, scroll with acceleration, move, drag |
| System | Volume, media keys, mic mute, brightness, Do Not Disturb / Focus, lock screen, screenshot |
| Apps | Open or focus an app, URL or file; window move/resize; menu-item click via Accessibility |
| Automation | Run a macOS Shortcut, trigger a Keyboard Maestro / BetterTouchTool / Hammerspoon macro |
| Control | Switch profile or layer, set a variable, toggle a state, panic / stop all |
| Messaging out | MIDI out and translation, OSC, local HTTP/WebSocket, opt-in outbound webhooks |
| Integrations | OBS, Lightroom, Home Assistant and more (see below) |
| Power (off by default) | AppleScript, shell. These need a separate opt-in and per-action trust, and stay disabled in imported packs until reviewed |

### Continuous controls
- Absolute knobs and faders with range, curve (linear, log, S-curve) and invert.
- Relative encoders in all common modes (two's complement, binary offset, sign-magnitude) with acceleration.
- **Soft takeover**: a non-motorised fader does nothing until it reaches the current value, so parameters never jump.
- "Turn → repeat keys": map knob detents to arrow keys or `J/K/L`. Every app becomes scrubbable, with no plugin needed.
- 14-bit CC and NRPN for smooth, high-resolution faders.

### Context
- Per-app profiles, switched by the frontmost app, with a global fallback profile.
- Conditions on bindings: app, Focus mode, time of day, variable, integration state.
- Manual profile lock ("stay on Streaming even when I click into Chrome").

### Feedback
- LED colour and blink per binding, driven by state (toggle on, active scene, current layer).
- Motor-fader and scribble-strip sync for Mackie-style surfaces.
- **HUD overlay**: hold a "?" pad to see a labelled map of the controller for the current profile; brief toasts on press.
- Menu-bar icon that shows the active profile and errors.

### Editor and UX
- Visual controller canvas; drag actions from a searchable library onto pads.
- Macro timeline editor (reorder, delays, holds, loops).
- **Press inspector**: a live trace that explains every press ("Note 36 → tap → profile Resolve → binding Blade → ⌘B ✓"), plus a "why didn't it fire?" explanation.
- Test-fire button, conflict detection, undo history, mapping search.
- Guided onboarding: detect the controller → permission checklist → suggested starter profile → first press in under 2 minutes.

### Trust, reliability, performance
- Signed, notarised, auto-updating (signed updates; update checks can be turned off).
- Panic pad and global panic shortcut; stuck-modifier recovery on every failure path.
- Latency budget: pad → keystroke under 10 ms p99; state change → LED under 50 ms.
- Diagnostics: dropped-event counter, latency histogram, exportable log with no keystroke contents.
- No telemetry by default. Network-using integrations are opt-in one by one and clearly labelled.

### Sharing
- Export/import profile packs (`.midimap`), with a readable summary of what a pack does before import.
- Community gallery ("Launchpad Mini × Resolve colour"). Packs live in a public, PR-reviewed repository; browsing it from the app is opt-in, and core use needs no account.
- Optional config folder location (iCloud Drive, Dropbox, a git repo) for multi-Mac setups.

---

## Roadmap: what ships in what order

The order follows dependencies. Nothing matters until mappings work with the window closed. The data model must widen *before* integrations arrive, or each one becomes a rewrite. Feedback needs the state store, and integrations need both.

### Phase 1 — v0.2 "Always on" (Checkpoints 0–2)
- Verify on real hardware (`03-test-plan.md`); fix what breaks.
- Menu-bar agent: the Rust service keeps running with the window hidden; launch at login.
- Permission onboarding: detect missing Accessibility access and deep-link to the setting.
- Panic stop, stuck-modifier recovery, recording reset on focus loss.
- Editable per-step delays.
- Signing, notarisation, DMG, auto-update.
- **Test harness**: create a virtual MIDI source in tests to drive the full pipeline without hardware.

**Acceptance:** a week of daily use with the window closed, no stuck keys, and a clean install on a fresh Mac.

### Phase 2 — v0.3–0.4 "Expressive"
- **Schema v2 + `Action` enum + migration.** This is the hinge for everything after it (see "Do this next").
- New actions: text, mouse, open app/URL, media/volume/mic, run Shortcut, switch profile.
- Gestures: tap / double / long-press / release / toggle / hold-repeat.
- Continuous controls: absolute, relative, 14-bit, soft takeover, "turn → repeat keys".
- Worker redesign: per-binding queue policy (`queue`, `drop_if_busy`, `restart`), so a long macro no longer blocks every other pad.
- Macro timeline editor.

**Acceptance:** v1 files migrate losslessly; one pad does tap = play/pause and hold = next track; a jog knob scrubs a timeline in Resolve and Final Cut via repeated keys.

### Phase 3 — v1.0 "Context-aware and lit" (public launch)
- Per-app profiles with auto-switching; layers and pages; conditions.
- Multiple simultaneous devices with stable identity.
- Device templates for the ~30 most common controllers, plus a visual canvas.
- State store + feedback engine: LEDs reflect toggles, layers and profile.
- HUD overlay and press inspector.
- Opt-in global recorder (native event tap) so `⌘Tab`-style shortcuts can be recorded, with explicit consent and the Input Monitoring permission.
- Profile pack import/export; starter packs for popular apps; onboarding flow.

**Acceptance:** a new user with a Launchpad gets a working, lit, per-app setup in under 5 minutes, with no documentation.

### Phase 4 — v1.x "Connected"
- **Integration protocol**: plugins run as separate processes speaking JSON-RPC over stdio (like LSP or MCP), with a manifest declaring actions, state, events and permissions. Built-in integrations use the same protocol.
- Flagship integrations: OBS, Lightroom Classic, Home Assistant, Spotify/Music, Zoom/Teams mute state.
- Virtual MIDI out + translation (cheap controller → DAW surface); OSC in/out.
- **Inbound API**: URL scheme, CLI, AppleScript dictionary, localhost HTTP/WebSocket with a token, App Intents for Shortcuts and Focus filters.
- **MCP server** on top of the same API, so AI assistants can read and propose mappings. Every write is shown to the user for confirmation.

**Acceptance:** the OBS mute pad reflects mic state within 50 ms even when muted from inside OBS; a third-party plugin ships without changes to core.

### Phase 5 — v2.0 "Everywhere"
- Community gallery browsing in-app; pack ratings; verified pack authors.
- iPad/iPhone companion controller (network MIDI or a local protocol).
- MIDI 2.0 / MIDI-CI auto-identification.
- Windows and Linux builds. Tauri, `midir` and `enigo` are already cross-platform; the OS-specific parts are permissions, the front-app watcher and system actions.
- Non-MIDI inputs: foot pedals and game controllers (HID), Stream Deck hardware, through the same pipeline.
- Studio deployment: managed profiles via MDM for broadcast and education rooms.

### Tracks that run through every phase
- **Reliability:** latency and drop metrics in CI with the virtual-MIDI harness.
- **Security:** typed actions over scripts, power actions opt-in, pack review, signed plugins, localhost-only API with a token.
- **Accessibility:** a VoiceOver-navigable editor, high-contrast HUD, large-target defaults; publish an accessibility starter pack.
- **Compatibility:** v1 keyboard mappings keep working, unchanged, through every release (Checkpoint 3's rule).

---

## Connecting to other apps

### Outbound: four ways MIDI Mapper drives other apps (least to most capable)

| Mechanism | Works with | Can it read state back (for LEDs)? |
|---|---|---|
| Keyboard / mouse injection (today) | Every app | No |
| OS automation: AppleScript / Apple Events, URL schemes, Shortcuts, Accessibility API, media keys | Most Mac apps | Sometimes (polling, menu-item check marks) |
| Native APIs and host-app plugins: WebSocket, HTTP, OSC, Lua/UXP/extension plugins | Apps with an API or SDK | Yes, in real time |
| MIDI itself: virtual ports, translation, Mackie Control | DAWs, lighting, VJ and show software | Yes |

### Inbound: how other apps drive MIDI Mapper

| Entry point | Used by |
|---|---|
| `midimapper://profile/switch?name=…` URL scheme | Raycast, Alfred, Keyboard Maestro, BetterTouchTool, browsers |
| App Intents / AppleScript | Shortcuts, Siri, Focus filters (switch profile when Focus changes) |
| `midimapper` CLI | Scripts, terminal users |
| Localhost HTTP / WebSocket (token-protected) | Stream Deck plugin, Bitfocus Companion, custom dashboards |
| MCP server | Claude and other AI assistants ("set up my Launchpad for Resolve colour grading") |
| Virtual MIDI input | Any MIDI-capable app or device |

### Integration catalog

| App | How | Two-way? | What users get | Phase |
|---|---|---|---|---|
| **OBS Studio** | obs-websocket v5 (built into OBS 28+) | Yes | Scene pads light the live scene; mute, record and stream pads show real state | 4 (flagship) |
| **Lightroom Classic** | Lua SDK plugin ↔ local socket (prior art: MIDI2LR) | Yes | Faders for develop sliders with soft takeover; pick/reject/next on pads | 4 |
| Photoshop, Premiere Pro | UXP plugin ↔ local API | Yes | Brush size/hardness knobs, tool pads, timeline actions | 4–5 |
| DaVinci Resolve | Shortcuts + scripting API (some external-script features need Studio) | Partial | Page switching, edit actions, colour via keys | 2 → 4 |
| Final Cut Pro | Shortcuts + Accessibility (prior art: CommandPost) | Partial | Edit pads, jog scrub, inspector navigation | 2 → 3 |
| Zoom, Teams, Meet | Global shortcuts + Accessibility read of mute state | Partial | A mute pad that tells the truth | 4 |
| Spotify, Apple Music | AppleScript | Yes (polling) | Play state on LED, volume fader, like/skip pads | 2 → 4 |
| Keynote, PowerPoint | AppleScript / keys | Partial | Presenter pads, slide number on display surfaces | 2 |
| macOS Shortcuts | `shortcuts run` out; App Intents in | Both | Anything Shortcuts can do, triggered from a pad | 2 (out), 4 (in) |
| Keyboard Maestro, BetterTouchTool, Hammerspoon | URL schemes / AppleScript | Both | Pads trigger existing power-user macros | 2 |
| Raycast, Alfred | Extension / workflow + URL scheme | Inbound | Switch profiles and fire bindings from the launcher | 4 |
| Ableton Live, Logic Pro, MainStage | Virtual MIDI, Mackie Control / HUI emulation | Yes | One controller for both DAW and desktop; layer per use | 4 |
| QLab, Resolume, TouchOSC | OSC + MIDI | Yes | Show control with state feedback | 4 |
| Home Assistant, Philips Hue, Elgato Key Light | Local HTTP / WebSocket APIs | Yes | Faders dim studio lights; pads show what's on | 4 |
| Bitfocus Companion | HTTP / OSC remote API | Yes | Gateway to broadcast gear (switchers, PTZ cameras) through Companion's module library | 4 |
| Elgato Stream Deck | Stream Deck plugin ↔ local API | Yes | Mixed setups: MIDI faders plus Stream Deck screens, one profile system | 4 |
| VS Code, JetBrains IDEs | Editor extension ↔ local API | Yes | Run/debug/step pads; test status on LED | 4 |
| n8n, Zapier, Make | Outbound webhook (opt-in network) | No | Cloud automations from a pad | 4 |
| Claude / AI assistants | MCP server | Both | Natural-language setup and "what does this pad do?" | 4 |

---

## Guardrails (stay true to the MVP's principles)

1. **Local-first.** Core features never need an account or network. Every network-using integration is opt-in and labelled.
2. **Consent.** Keyboard capture only when armed; global capture only by explicit choice; permissions are explained before they are requested.
3. **Typed actions over scripts.** Shell and AppleScript are a deliberate, separately-enabled tier, never the default way to do something.
4. **Imported things are untrusted.** Packs and plugins show what they will do; power actions and network access stay off until the user approves them.
5. **The panic button always works**, and modifiers never stay stuck.
6. **Never lose a mapping.** Every schema change ships with a migration and a backup.

---

## Do this next

1. **Finish Checkpoint 0 on real hardware.** The README notes the starter has not been validated against a physical controller yet.
2. **Land schema v2 before any new feature.** `Mapping.steps: Vec<KeyChord>` (`model.rs:36`) is hard-wired into `engine::playback` (`engine.rs:122`) and `App.tsx`. Replacing it with `actions: Vec<Action>`, where `Action::Keys` wraps today's chords, plus a v1 → v2 migration, is cheap now and expensive after five integrations.
3. **Unblock the worker.** `start_action_worker` (`lib.rs:80`) runs playback inline, including the per-step `thread::sleep`, so a long macro delays every later press and the UI monitor. Move execution to per-binding tasks with a queue policy. Replace the per-event `snapshot().mappings` clone (`lib.rs:95`) with an index keyed by device + control.
4. **Ship the menu-bar agent** (Checkpoint 2). It is the first feature real users will notice.
5. **Add the virtual-MIDI test harness** so every later phase gets end-to-end tests without hardware.
