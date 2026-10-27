# 01 — Product Requirements (MVP)

## Problem

Ordinary MIDI pads and buttons are convenient tactile inputs, but typical workflows require musical software or MIDI-specific knowledge. Make MIDI controllers programmable for ordinary macOS keyboard tasks without manually picking an action from a menu.

## Primary user story

> As a Mac user with a MIDI controller, I select or learn a physical button, press **Record Keyboard Mapping**, perform one or more keyboard shortcuts, and press **Confirm Mapping**. Pressing the MIDI button afterwards replays those shortcuts in order.

## Core journey

1. Choose one MIDI input device.
2. Click **Learn MIDI Control**; physically press a pad/button. Input is identified by kind, channel, and note or CC number.
3. Enter an optional human-readable mapping label.
4. Click **Record Keyboard Mapping**. Window keyboard capture becomes active and existing keyboard replay is paused.
5. Press shortcut(s). The UI shows the order of recorded chords. User can undo the most recent step.
6. **Confirm Mapping** writes the mapping; **Cancel** discards draft without touching previous saved mapping.
7. Press MIDI control with target app focused to replay actions.

## Functional requirements

| ID | Behavior | Priority |
|---|---|---|
| F1 | List available CoreMIDI devices | Must |
| F2 | Select one device and reconnect after temporary disappearance | Must |
| F3 | Learn Note and CC button controls from physical input | Must |
| F4 | Capture supported keyboard shortcuts in window until explicit confirmation | Must |
| F5 | Confirm, cancel, undo last keyboard step | Must |
| F6 | Save, replace, and remove mappings locally | Must |
| F7 | Playback discrete key chords in order | Must |
| F8 | Display last MIDI event and playback errors | Must |
| F9 | Continuous knobs and sliders for volume/scrolling | Later |
| F10 | App-aware profiles, macro delays, key hold, menu-bar agent | Later |

## Edge cases

- Note Off never triggers playback.
- Note On with velocity zero is release.
- CC value must cross from below 64 to at least 64 to trigger.
- If recording is active, mapped actions are suppressed.
- Unsupported keyboard keys display a message and are not recorded.
- Maximum 32 keyboard steps per MIDI input.
- If Accessibility permission is missing, show an actionable replay error.
- If the selected MIDI controller disappears, the app should remain responsive and attempt to reconnect.
- Duplicate device names may exist; MIDI port ID is used for the MVP, but CoreMIDI's ID stability must be verified on actual hardware.

## Nonfunctional requirements

- No cloud services, subscriptions, login, telemetry, or network access for mapping data.
- Keep MIDI input callback small and non-blocking.
- Prevent modifier keys from staying pressed when an action fails.
- Persist only explicitly confirmed mappings.
- Clear consent model: keyboard capture only after user selects Record and while app window is active.

## Boundaries

This prototype is **not** a general system-wide recorder, full DAW control surface, continuous MIDI automation engine, or complete Stream Deck replacement. It intentionally supports discrete keyboard input first.
