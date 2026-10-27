use std::{thread, time::Duration};

use enigo::{Direction::{Click, Press, Release}, Enigo, Key, Keyboard, Settings};

use crate::model::{KeyChord, Mapping};

fn is_modifier(key: &str) -> bool {
    matches!(key, "Meta" | "Control" | "Alt" | "Shift")
}

/// Only accept keyboard codes we can replay predictably in the MVP.
pub fn supported_key(code: &str) -> bool {
    let letter = code.strip_prefix("Key").is_some_and(|s| s.len() == 1 && s.bytes().all(|b| b.is_ascii_uppercase()));
    let digit = code.strip_prefix("Digit").is_some_and(|s| s.len() == 1 && s.bytes().all(|b| b.is_ascii_digit()));
    let fn_key = code.strip_prefix('F').and_then(|s| s.parse::<u8>().ok()).is_some_and(|n| (1..=12).contains(&n));
    letter || digit || fn_key || matches!(code,
        "Space" | "Enter" | "NumpadEnter" | "Tab" | "Escape" | "Backspace" |
        "Delete" | "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" |
        "Home" | "End" | "PageUp" | "PageDown" |
        "Minus" | "Equal" | "BracketLeft" | "BracketRight" | "Backslash" |
        "Semicolon" | "Quote" | "Comma" | "Period" | "Slash" | "Backquote")
}

pub fn validate_mapping(mapping: &Mapping) -> Result<(), String> {
    if mapping.device_id.trim().is_empty() || mapping.label.trim().is_empty() {
        return Err("Device ID and label are required".into());
    }
    if mapping.label.chars().count() > 80 { return Err("Label is too long".into()); }
    if !(1..=16).contains(&mapping.control.channel) || mapping.control.number > 127 {
        return Err("Invalid MIDI channel or note/controller number".into());
    }
    if mapping.steps.is_empty() || mapping.steps.len() > 32 {
        return Err("Record between 1 and 32 keyboard steps".into());
    }
    for step in &mapping.steps {
        if step.keys.len() > 5 || step.keys.is_empty() || step.delay_after_ms > 5_000 {
            return Err("Invalid chord length or delay".into());
        }
        let main_keys: Vec<_> = step.keys.iter().filter(|k| !is_modifier(k)).collect();
        if main_keys.len() != 1 || !supported_key(main_keys[0]) {
            return Err(format!("Unsupported key chord: {:?}", step.keys));
        }
        let mods: Vec<_> = step.keys.iter().filter(|k| is_modifier(k)).collect();
        let mut unique = std::collections::HashSet::new();
        if !mods.iter().all(|m| unique.insert(m.as_str())) {
            return Err("Repeated modifier in a chord".into());
        }
    }
    Ok(())
}

/// macOS virtual keycode (kVK_ANSI_*) for a physical character key, as named by KeyboardEvent.code.
/// Raw keycodes avoid enigo's Key::Unicode layout lookup, which macOS only permits on the main thread.
fn ansi_keycode(code: &str) -> Option<u32> {
    Some(match code {
        "KeyA" => 0x00, "KeyS" => 0x01, "KeyD" => 0x02, "KeyF" => 0x03, "KeyH" => 0x04,
        "KeyG" => 0x05, "KeyZ" => 0x06, "KeyX" => 0x07, "KeyC" => 0x08, "KeyV" => 0x09,
        "KeyB" => 0x0B, "KeyQ" => 0x0C, "KeyW" => 0x0D, "KeyE" => 0x0E, "KeyR" => 0x0F,
        "KeyY" => 0x10, "KeyT" => 0x11, "Digit1" => 0x12, "Digit2" => 0x13, "Digit3" => 0x14,
        "Digit4" => 0x15, "Digit6" => 0x16, "Digit5" => 0x17, "Equal" => 0x18, "Digit9" => 0x19,
        "Digit7" => 0x1A, "Minus" => 0x1B, "Digit8" => 0x1C, "Digit0" => 0x1D, "BracketRight" => 0x1E,
        "KeyO" => 0x1F, "KeyU" => 0x20, "BracketLeft" => 0x21, "KeyI" => 0x22, "KeyP" => 0x23,
        "KeyL" => 0x25, "KeyJ" => 0x26, "Quote" => 0x27, "KeyK" => 0x28, "Semicolon" => 0x29,
        "Backslash" => 0x2A, "Comma" => 0x2B, "Slash" => 0x2C, "KeyN" => 0x2D, "KeyM" => 0x2E,
        "Period" => 0x2F, "Backquote" => 0x32,
        _ => return None,
    })
}

fn key_from_code(code: &str) -> Result<Key, String> {
    if let Some(keycode) = ansi_keycode(code) {
        return Ok(Key::Other(keycode));
    }
    Ok(match code {
        "Space" => Key::Space,
        "Enter" | "NumpadEnter" => Key::Return,
        "Tab" => Key::Tab,
        "Escape" => Key::Escape,
        "Backspace" => Key::Backspace,
        "Delete" => Key::Delete,
        "ArrowUp" => Key::UpArrow,
        "ArrowDown" => Key::DownArrow,
        "ArrowLeft" => Key::LeftArrow,
        "ArrowRight" => Key::RightArrow,
        "Home" => Key::Home,
        "End" => Key::End,
        "PageUp" => Key::PageUp,
        "PageDown" => Key::PageDown,
        "F1" => Key::F1, "F2" => Key::F2, "F3" => Key::F3,
        "F4" => Key::F4, "F5" => Key::F5, "F6" => Key::F6,
        "F7" => Key::F7, "F8" => Key::F8, "F9" => Key::F9,
        "F10" => Key::F10, "F11" => Key::F11, "F12" => Key::F12,
        _ => return Err(format!("Unsupported key: {code}")),
    })
}

fn play_chord(enigo: &mut Enigo, chord: &KeyChord) -> Result<(), String> {
    let mut held: Vec<Key> = Vec::new();
    for modifier in ["Meta", "Control", "Alt", "Shift"] {
        if chord.keys.iter().any(|s| s == modifier) {
            let key = match modifier {
                "Meta" => Key::Meta,
                "Control" => Key::Control,
                "Alt" => Key::Alt,
                _ => Key::Shift,
            };
            if let Err(e) = enigo.key(key, Press) {
                for k in held.iter().rev() { let _ = enigo.key(*k, Release); }
                return Err(format!("Failed to press modifier: {e}"));
            }
            held.push(key);
        }
    }

    let main_code = chord.keys.iter().find(|k| !is_modifier(k)).ok_or("Missing main key")?;
    let result = key_from_code(main_code).and_then(|key| enigo.key(key, Click).map_err(|e| e.to_string()));
    // Always release pressed modifiers, even on an action error.
    for key in held.iter().rev() { let _ = enigo.key(*key, Release); }
    result
}

pub fn playback(mapping: &Mapping) -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| format!("Input initialization failed: {e}. Check Accessibility permission."))?;
    for step in &mapping.steps {
        play_chord(&mut enigo, step)?;
        thread::sleep(Duration::from_millis(step.delay_after_ms));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ControlKind, MidiControl};

    #[test]
    fn rejects_unsupported_codes() {
        let mut m = Mapping {
            device_id: "port".into(),
            control: MidiControl { kind: ControlKind::Note, channel: 1, number: 36 },
            label: "Copy".into(),
            steps: vec![KeyChord { keys: vec!["Meta".into(), "KeyC".into()], delay_after_ms: 80 }],
        };
        assert!(validate_mapping(&m).is_ok());
        m.steps[0].keys = vec!["Meta".into(), "Unidentified".into()];
        assert!(validate_mapping(&m).is_err());
    }

    #[test]
    fn supported_codes_never_need_layout_lookup() {
        // Key::Unicode makes enigo query the keyboard layout, which macOS only allows on the
        // main thread; playback runs on a worker thread, so it would abort the app.
        let codes = ('A'..='Z').map(|c| format!("Key{c}"))
            .chain(('0'..='9').map(|c| format!("Digit{c}")))
            .chain((1..=12).map(|n| format!("F{n}")))
            .chain(["Space", "Enter", "NumpadEnter", "Tab", "Escape", "Backspace", "Delete",
                "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End", "PageUp", "PageDown",
                "Minus", "Equal", "BracketLeft", "BracketRight", "Backslash", "Semicolon", "Quote",
                "Comma", "Period", "Slash", "Backquote"].map(String::from));
        for code in codes {
            assert!(supported_key(&code), "{code} should be supported");
            let key = key_from_code(&code).unwrap_or_else(|e| panic!("{code}: {e}"));
            assert!(!matches!(key, Key::Unicode(_)), "{code} maps to a layout-dependent key");
        }
        assert!(matches!(key_from_code("KeyC"), Ok(Key::Other(0x08))));
        assert!(matches!(key_from_code("KeyV"), Ok(Key::Other(0x09))));
    }
}
