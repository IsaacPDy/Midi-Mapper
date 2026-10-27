mod engine;
mod model;

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{atomic::{AtomicBool, Ordering}, mpsc::{self, SyncSender}, Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use midir::{MidiInput, MidiInputConnection};
use tauri::{Emitter, Manager, State};

use crate::model::{ControlKind, DeviceInfo, Mapping, MidiControl, MidiEvent, StoredData};

struct Shared {
    settings: Mutex<StoredData>,
    capture_active: AtomicBool,
    config_file: PathBuf,
    startup_notice: Mutex<Option<String>>,
}

/// Load the mappings file. An unreadable or unsupported file is moved aside to
/// `mappings.json.bad-<unix-ts>` so the app still starts; the returned notice says what happened.
fn load_settings(config_file: &Path) -> Result<(StoredData, Option<String>), String> {
    if !config_file.exists() {
        return Ok((StoredData::default(), None));
    }
    let bytes = fs::read(config_file).map_err(|e| e.to_string())?;
    let problem = match serde_json::from_slice::<StoredData>(&bytes) {
        Ok(data) if data.schema_version == 1 => return Ok((data, None)),
        Ok(data) => format!("it uses unsupported schema version {}", data.schema_version),
        Err(e) => format!("it could not be parsed ({e})"),
    };
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let backup = config_file.with_extension(format!("json.bad-{stamp}"));
    fs::rename(config_file, &backup).map_err(|e| e.to_string())?;
    let notice = format!(
        "Your mappings file was not usable: {problem}. It was saved as {} and the app started with no mappings.",
        backup.display()
    );
    Ok((StoredData::default(), Some(notice)))
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

impl Shared {
    fn snapshot(&self) -> StoredData {
        self.settings.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn update(&self, edit: impl FnOnce(&mut StoredData)) -> Result<(), String> {
        let mut settings = self.settings.lock().map_err(|e| e.to_string())?;
        let mut changed = settings.clone();
        edit(&mut changed);
        let serialized = serde_json::to_vec_pretty(&changed).map_err(|e| e.to_string())?;
        let temp = self.config_file.with_extension("json.tmp");
        fs::write(&temp, serialized).map_err(|e| e.to_string())?;
        fs::rename(&temp, &self.config_file).map_err(|e| e.to_string())?;
        *settings = changed;
        Ok(())
    }
}

#[tauri::command]
fn get_settings(shared: State<'_, Arc<Shared>>) -> StoredData {
    shared.snapshot()
}

#[tauri::command]
fn take_startup_notice(shared: State<'_, Arc<Shared>>) -> Option<String> {
    shared.startup_notice.lock().unwrap_or_else(|e| e.into_inner()).take()
}

#[tauri::command]
fn check_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    { unsafe { AXIsProcessTrusted() } }
    #[cfg(not(target_os = "macos"))]
    { true }
}

#[tauri::command]
fn open_accessibility_settings() -> Result<(), String> {
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_devices() -> Result<Vec<DeviceInfo>, String> {
    let input = MidiInput::new("MIDI Mapper discovery").map_err(|e| e.to_string())?;
    Ok(input.ports().iter().map(|port| DeviceInfo {
        id: port.id(),
        name: input.port_name(port).unwrap_or_else(|_| "Unnamed MIDI input".into()),
    }).collect())
}

#[tauri::command]
fn select_device(device_id: Option<String>, shared: State<'_, Arc<Shared>>) -> Result<(), String> {
    shared.update(|data| data.selected_device_id = device_id)
}

#[tauri::command]
fn set_capture_active(active: bool, shared: State<'_, Arc<Shared>>) {
    shared.capture_active.store(active, Ordering::SeqCst);
}

#[tauri::command]
fn save_mapping(mapping: Mapping, shared: State<'_, Arc<Shared>>) -> Result<(), String> {
    engine::validate_mapping(&mapping)?;
    shared.update(|data| {
        data.mappings.retain(|m| !(m.device_id == mapping.device_id && m.control == mapping.control));
        data.mappings.push(mapping);
    })
}

#[tauri::command]
fn delete_mapping(device_id: String, control: MidiControl, shared: State<'_, Arc<Shared>>) -> Result<(), String> {
    shared.update(|data| data.mappings.retain(|m| !(m.device_id == device_id && m.control == control)))
}

fn start_action_worker(app: tauri::AppHandle, shared: Arc<Shared>, receiver: mpsc::Receiver<MidiEvent>) {
    thread::spawn(move || {
        // A CC-type button triggers only when it crosses into the high range.
        let mut cc_high: HashMap<(String, u8, u8), bool> = HashMap::new();
        while let Ok(event) = receiver.recv() {
            let _ = app.emit("midi-input", &event);
            let trigger = match event.control.kind {
                ControlKind::Note => event.is_press,
                ControlKind::Cc => {
                    let key = (event.device_id.clone(), event.control.channel, event.control.number);
                    let was_high = cc_high.insert(key, event.is_press).unwrap_or(false);
                    event.is_press && !was_high
                }
            };
            if !trigger || shared.capture_active.load(Ordering::SeqCst) { continue; }
            let mapping = shared.snapshot().mappings.into_iter().find(|m| {
                m.device_id == event.device_id && m.control == event.control
            });
            if let Some(mapping) = mapping {
                let result = engine::playback(&mapping);
                match result {
                    Ok(()) => { let _ = app.emit("mapping-triggered", &mapping.label); }
                    Err(error) => { let _ = app.emit("playback-error", error); }
                }
            }
        }
    });
}

fn start_midi_monitor(shared: Arc<Shared>, sender: SyncSender<MidiEvent>) {
    thread::spawn(move || {
        let mut connected_to: Option<String> = None;
        let mut connection: Option<MidiInputConnection<()>> = None;
        loop {
            let desired = shared.snapshot().selected_device_id;
            let probe = MidiInput::new("MIDI Mapper port monitor");
            if let Ok(input) = probe {
                let ports = input.ports();
                let found = desired.as_ref().and_then(|id| ports.iter().find(|p| p.id() == id.as_str()));
                let must_disconnect = desired != connected_to || (connection.is_some() && found.is_none());
                if must_disconnect {
                    connection = None;
                    connected_to = None;
                }
                if connection.is_none() {
                    if let (Some(id), Some(port)) = (desired.as_ref(), found) {
                        let source_id = id.clone();
                        let tx = sender.clone();
                        let opened = input.connect(port, "MIDI Mapper input", move |_stamp, bytes, _data| {
                            if let Some(event) = model::decode_message(&source_id, bytes) {
                                // The MIDI callback never performs keyboard I/O or blocks on UI.
                                let _ = tx.try_send(event);
                            }
                        }, ());
                        if let Ok(conn) = opened {
                            connection = Some(conn);
                            connected_to = Some(id.clone());
                        }
                    }
                }
            } else {
                connection = None;
                connected_to = None;
            }
            thread::sleep(Duration::from_millis(1_500));
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // CoreMIDI delivers device add/remove notifications to the run loop of the thread that
            // created the process's first MIDI client. Create it here on the main thread (which runs
            // the app's run loop) and keep it alive; otherwise the monitor thread creates it first and
            // devices plugged in after launch never appear.
            std::mem::forget(MidiInput::new("MIDI Mapper hot-plug anchor").map_err(|e| e.to_string())?);
            let folder = app.path().app_config_dir().map_err(|e| e.to_string())?;
            fs::create_dir_all(&folder)?;
            let config_file = folder.join("mappings.json");
            let (data, notice) = load_settings(&config_file)?;
            let shared = Arc::new(Shared {
                settings: Mutex::new(data),
                capture_active: AtomicBool::new(false),
                config_file,
                startup_notice: Mutex::new(notice),
            });
            let (tx, rx) = mpsc::sync_channel::<MidiEvent>(128);
            start_action_worker(app.handle().clone(), shared.clone(), rx);
            start_midi_monitor(shared.clone(), tx);
            app.manage(shared);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings, list_devices, select_device, set_capture_active, save_mapping, delete_mapping,
            take_startup_notice, check_accessibility, open_accessibility_settings
        ])
        .run(tauri::generate_context!())
        .expect("MIDI Mapper could not start");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("midi-mapper-test-{}-{name}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir.join("mappings.json")
    }

    #[test]
    fn missing_file_gives_defaults_without_notice() {
        let file = temp_file("missing");
        let (data, notice) = load_settings(&file).unwrap();
        assert!(data.mappings.is_empty());
        assert!(notice.is_none());
    }

    #[test]
    fn corrupt_file_is_moved_aside() {
        let file = temp_file("corrupt");
        fs::write(&file, b"not json").unwrap();
        let (data, notice) = load_settings(&file).unwrap();
        assert!(data.mappings.is_empty());
        assert!(notice.unwrap().contains("could not be parsed"));
        assert!(!file.exists());
        let backups = fs::read_dir(file.parent().unwrap()).unwrap()
            .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().contains(".bad-"))
            .count();
        assert_eq!(backups, 1);
    }

    #[test]
    fn unsupported_schema_version_is_moved_aside() {
        let file = temp_file("schema");
        fs::write(&file, br#"{"schema_version":2,"selected_device_id":null,"mappings":[]}"#).unwrap();
        let (_, notice) = load_settings(&file).unwrap();
        assert!(notice.unwrap().contains("schema version 2"));
        assert!(!file.exists());
    }

    #[test]
    fn valid_file_loads_untouched() {
        let file = temp_file("valid");
        fs::write(&file, br#"{"schema_version":1,"selected_device_id":"x","mappings":[]}"#).unwrap();
        let (data, notice) = load_settings(&file).unwrap();
        assert_eq!(data.selected_device_id.as_deref(), Some("x"));
        assert!(notice.is_none());
        assert!(file.exists());
    }
}
