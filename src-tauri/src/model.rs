use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ControlKind {
    Note,
    Cc,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MidiControl {
    pub kind: ControlKind,
    /// MIDI channel in the human-friendly 1..=16 range.
    pub channel: u8,
    /// Note number or controller number, 0..=127.
    pub number: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyChord {
    /// Modifiers: Meta, Control, Alt, Shift. Main key: KeyboardEvent.code.
    pub keys: Vec<String>,
    #[serde(default = "default_delay")]
    pub delay_after_ms: u64,
}

fn default_delay() -> u64 {
    80
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapping {
    pub device_id: String,
    pub control: MidiControl,
    pub label: String,
    pub steps: Vec<KeyChord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredData {
    pub schema_version: u32,
    pub selected_device_id: Option<String>,
    pub mappings: Vec<Mapping>,
}

impl Default for StoredData {
    fn default() -> Self {
        Self {
            schema_version: 1,
            selected_device_id: None,
            mappings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MidiEvent {
    pub device_id: String,
    pub control: MidiControl,
    pub value: u8,
    pub is_press: bool,
}

/// Decode MIDI 1.0 channel-voice notes and CC; ignore other message families.
/// Treat Note On velocity 0 as release (same as Note Off).
pub fn decode_message(device_id: &str, message: &[u8]) -> Option<MidiEvent> {
    if message.len() < 3 {
        return None;
    }
    let status = message[0];
    if !(0x80..=0xEF).contains(&status) || message[1] > 127 || message[2] > 127 {
        return None;
    }
    let channel = (status & 0x0F) + 1;
    let (kind, is_press) = match status & 0xF0 {
        0x80 => (ControlKind::Note, false),
        0x90 => (ControlKind::Note, message[2] > 0),
        0xB0 => (ControlKind::Cc, message[2] >= 64),
        _ => return None,
    };
    Some(MidiEvent {
        device_id: device_id.to_string(),
        control: MidiControl { kind, channel, number: message[1] },
        value: message[2],
        is_press,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_on_and_zero_velocity() {
        let on = decode_message("dev", &[0x90, 36, 127]).unwrap();
        assert_eq!(on.control.channel, 1);
        assert_eq!(on.control.number, 36);
        assert!(on.is_press);
        let off = decode_message("dev", &[0x90, 36, 0]).unwrap();
        assert!(!off.is_press);
        assert!(!decode_message("dev", &[0x80, 36, 0]).unwrap().is_press);
    }

    #[test]
    fn distinguishes_channel_cc_and_bad_packets() {
        let cc = decode_message("dev", &[0xB2, 12, 77]).unwrap();
        assert_eq!(cc.control.channel, 3);
        assert!(cc.is_press);
        assert_eq!(cc.control.kind, ControlKind::Cc);
        assert!(decode_message("dev", &[0xF0, 1, 2]).is_none());
        assert!(decode_message("dev", &[0x90, 1]).is_none());
    }
}
