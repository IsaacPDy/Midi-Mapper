import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { MidiControl, MidiDevice, MidiEvent, Mapping, KeyChord, StoredData } from './types';
import { controlKey, formatKey, isModifier, supportedKey } from './types';

type Mode = 'idle' | 'learning' | 'recording';
const EMPTY: StoredData = { schema_version: 1, selected_device_id: null, mappings: [] };
const DEFAULT_DELAY = 80;
const controlText = (control: MidiControl) =>
  `${control.kind === 'note' ? 'Note' : 'CC'} ${control.number} · Ch ${control.channel}`;

export default function App() {
  const [settings, setSettings] = useState<StoredData>(EMPTY);
  const [devices, setDevices] = useState<MidiDevice[]>([]);
  const [selected, setSelected] = useState<MidiControl | null>(null);
  const [mode, setMode] = useState<Mode>('idle');
  const modeRef = useRef<Mode>('idle');
  const [draft, setDraft] = useState<KeyChord[]>([]);
  const [label, setLabel] = useState('New mapping');
  const [lastMessage, setLastMessage] = useState<MidiEvent | null>(null);
  const [message, setMessage] = useState('Select a MIDI device to begin.');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  const [accessOk, setAccessOk] = useState<boolean | null>(null);
  const [editSteps, setEditSteps] = useState<KeyChord[] | null>(null);
  const captureRef = useRef<HTMLDivElement | null>(null);
  const selectedDevice = settings.selected_device_id;

  const changeMode = useCallback((next: Mode) => { modeRef.current = next; setMode(next); }, []);
  const refresh = useCallback(async () => {
    const saved = await invoke<StoredData>('get_settings');
    setSettings(saved);
  }, []);
  const refreshDevices = useCallback(async () => {
    try { setDevices(await invoke<MidiDevice[]>('list_devices')); }
    catch (e) { setError(`Could not read MIDI devices: ${String(e)}`); }
  }, []);

  const checkAccess = useCallback(async () => {
    try { setAccessOk(await invoke<boolean>('check_accessibility')); }
    catch { setAccessOk(null); }
  }, []);

  useEffect(() => {
    // A reload can leave the backend flag set from a previous session, which silently mutes every pad.
    void invoke('set_capture_active', { active: false }).catch(e => setError(String(e)));
    void invoke<string | null>('take_startup_notice').then(n => { if (n) setNotice(n); }).catch(() => {});
    void refresh().catch(e => setError(`Unable to load settings: ${String(e)}`));
    void refreshDevices();
    void checkAccess();
    const interval = setInterval(() => void refreshDevices(), 3500);
    window.addEventListener('focus', checkAccess);
    return () => { clearInterval(interval); window.removeEventListener('focus', checkAccess); };
  }, [refresh, refreshDevices, checkAccess]);

  useEffect(() => {
    let alive = true;
    let unlistenMidi: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;
    let unlistenSuccess: (() => void) | undefined;
    (async () => {
      const stopMidi = await listen<MidiEvent>('midi-input', e => {
        if (!alive) return;
        const event = e.payload;
        setLastMessage(event);
        if (modeRef.current === 'learning' && event.is_press) {
          setSelected(event.control);
          setDraft([]);
          setEditSteps(null);
          setLabel(`${event.control.kind === 'note' ? 'Pad' : 'CC'} ${event.control.number}`);
          changeMode('idle');
          void invoke('set_capture_active', { active: false }).catch(err => setError(String(err)));
          setMessage(`Learned ${controlText(event.control)}. Record a keyboard mapping next.`);
        }
      });
      if (!alive) { stopMidi(); return; }
      unlistenMidi = stopMidi;
      unlistenError = await listen<string>('playback-error', e => setError(e.payload));
      unlistenSuccess = await listen<string>('mapping-triggered', e => setMessage(`Triggered ${e.payload}`));
    })().catch(e => setError(`Cannot subscribe to MIDI events: ${String(e)}`));
    return () => { alive = false; unlistenMidi?.(); unlistenError?.(); unlistenSuccess?.(); };
  }, [changeMode]);

  // Deliberately window-scoped: capture only while this editor is open and armed.
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (modeRef.current !== 'recording') return;
      e.preventDefault();
      e.stopPropagation();
      if (e.repeat || isModifier(e.key)) return;
      if (!supportedKey(e.code)) {
        setMessage(`Unsupported key: ${e.code}. Press a standard key or shortcut.`);
        return;
      }
      const modifiers = [
        ...(e.metaKey ? ['Meta'] : []),
        ...(e.ctrlKey ? ['Control'] : []),
        ...(e.altKey ? ['Alt'] : []),
        ...(e.shiftKey ? ['Shift'] : []),
      ];
      setDraft(existing => existing.length < 32
        ? [...existing, { keys: [...modifiers, e.code], delay_after_ms: DEFAULT_DELAY }]
        : existing);
    }
    function onKeyUp(e: KeyboardEvent) {
      if (modeRef.current !== 'recording') return;
      e.preventDefault();
      e.stopPropagation();
    }
    // Keystrokes go to another window once focus is lost, so a half-recorded sequence is discarded.
    function onBlur() {
      if (modeRef.current !== 'recording') return;
      modeRef.current = 'idle';
      setMode('idle');
      setDraft([]);
      setMessage('Recording canceled because the window lost focus; saved mapping unchanged.');
      void invoke('set_capture_active', { active: false }).catch(err => setError(String(err)));
    }
    window.addEventListener('keydown', onKeyDown, true);
    window.addEventListener('keyup', onKeyUp, true);
    window.addEventListener('blur', onBlur);
    return () => {
      window.removeEventListener('keydown', onKeyDown, true);
      window.removeEventListener('keyup', onKeyUp, true);
      window.removeEventListener('blur', onBlur);
    };
  }, []);

  const mappings = settings.mappings.filter(m => m.device_id === selectedDevice);
  const existing = selected ? mappings.find(m => controlKey(m.control) === controlKey(selected)) : undefined;
  const shown = mode === 'recording' ? draft : editSteps ?? existing?.steps ?? [];
  const dirty = Boolean(existing) && mode === 'idle' && (label.trim() !== existing!.label || editSteps !== null);
  const currentDevice = devices.find(d => d.id === selectedDevice);
  const selectedConnected = Boolean(currentDevice);

  async function stopCapture() {
    changeMode('idle');
    await invoke('set_capture_active', { active: false });
  }
  async function chooseDevice(id: string) {
    try {
      setError(''); await stopCapture();
      await invoke('select_device', { deviceId: id || null });
      setSelected(null); setDraft([]); setEditSteps(null); setLastMessage(null);
      await refresh();
      setMessage(id ? 'Device selected. Click Learn MIDI Control and press a pad.' : 'Choose a MIDI device.');
    } catch (e) { setError(String(e)); }
  }
  async function learn() {
    if (!selectedDevice) return;
    try {
      setError('');
      await invoke('set_capture_active', { active: true });
      changeMode('learning');
      setMessage('Press a pad or button on your MIDI controller.');
    } catch (e) { setError(String(e)); }
  }
  function chooseMapping(mapping: Mapping) {
    if (mode !== 'idle') return;
    setSelected(mapping.control);
    setLabel(mapping.label);
    setDraft([]);
    setEditSteps(null);
    setError('');
  }
  function setDelay(index: number, raw: string) {
    const ms = Math.max(0, Math.min(5000, Math.round(Number(raw)) || 0));
    const update = (steps: KeyChord[]) => steps.map((s, i) => i === index ? { ...s, delay_after_ms: ms } : s);
    if (mode === 'recording') setDraft(update);
    else if (mode === 'idle' && existing) setEditSteps(update(editSteps ?? existing.steps));
  }
  async function beginRecording() {
    if (!selected) return;
    try {
      setError('');
      await invoke('set_capture_active', { active: true });
      setDraft([]);
      changeMode('recording');
      setMessage('Press your keyboard shortcuts. Nothing is saved until you confirm.');
      captureRef.current?.focus();
    } catch (e) { setError(String(e)); }
  }
  async function confirmMapping() {
    if (!selected || !selectedDevice || !draft.length || busy) return;
    setBusy(true);
    try {
      const mapping: Mapping = { device_id: selectedDevice, control: selected, label: label.trim() || controlText(selected), steps: draft };
      await invoke('save_mapping', { mapping });
      await stopCapture();
      await refresh();
      setDraft([]);
      setEditSteps(null);
      setError('');
      setMessage(`${mapping.label} saved. Press your MIDI control to replay it.`);
    } catch (e) { setError(`Failed to save mapping: ${String(e)}`); }
    finally { setBusy(false); }
  }
  async function saveChanges() {
    if (!existing || busy) return;
    setBusy(true);
    try {
      const mapping: Mapping = { ...existing, label: label.trim() || controlText(existing.control), steps: editSteps ?? existing.steps };
      await invoke('save_mapping', { mapping });
      await refresh();
      setLabel(mapping.label);
      setEditSteps(null);
      setError('');
      setMessage(`${mapping.label} updated.`);
    } catch (e) { setError(`Failed to save mapping: ${String(e)}`); }
    finally { setBusy(false); }
  }
  async function cancelCapture(text: string) {
    try { await stopCapture(); setDraft([]); setMessage(text); }
    catch (e) { setError(String(e)); }
  }
  async function clearMapping() {
    if (!existing) return;
    try {
      await invoke('delete_mapping', { deviceId: existing.device_id, control: existing.control });
      await refresh(); setDraft([]); setEditSteps(null); setMessage('Mapping deleted.'); setError('');
    } catch (e) { setError(String(e)); }
  }

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><span className="brand-icon">◈</span><span>MIDI Mapper<small>macOS controller utility</small></span></div>
      <div className="nav-title">WORKSPACE</div>
      <div className="nav-current">▦ &nbsp; Controls</div>
      <div className="nav-title top-gap">INPUT DEVICE</div>
      <select className="device-select" aria-label="MIDI input device" value={selectedDevice ?? ''} onChange={e => void chooseDevice(e.target.value)}>
        <option value="">Select a device…</option>
        {devices.map(d => <option value={d.id} key={d.id}>{d.name}</option>)}
        {selectedDevice && !selectedConnected && <option value={selectedDevice}>Disconnected device</option>}
      </select>
      <div className="device-state"><span className={`dot ${selectedConnected ? 'green' : ''}`} />{selectedConnected ? 'Device available' : 'Not detected'}</div>
      <button className="link-button" onClick={() => void refreshDevices()}>↻ Refresh device list</button>
      <div className="sidebar-bottom"><strong>Local-first</strong><p>Mappings are stored on this Mac. No account or cloud service required.</p></div>
    </aside>
    <main className="main">
      <header className="main-head"><div><div className="eyebrow">CONTROLS / KEYBOARD MAPPINGS</div><h1>Make MIDI do more.</h1><p>Turn MIDI pads and buttons into keyboard shortcuts and sequences.</p></div><div className="version">MVP · 0.1.0</div></header>
      {error && <div className="alert error" role="alert"><span>⚠ {error}</span><button onClick={() => setError('')}>Dismiss</button></div>}
      {notice && <div className="alert warn" role="status"><span>{notice}</span><button onClick={() => setNotice('')}>Dismiss</button></div>}
      {accessOk === false && <div className="alert warn" role="alert"><span>MIDI Mapper needs Accessibility permission to press keys for you. Turn it on in System Settings, then return to this window.</span><button onClick={() => void invoke('open_accessibility_settings').catch(e => setError(String(e)))}>Open System Settings</button></div>}
      <div className="layout">
        <section className="panel mapping-list">
          <div className="panel-heading"><h2>Your controls</h2><span className="count">{mappings.length}</span></div>
          <p className="helper">Select a saved control or learn a new one.</p>
          <button className={`learn-button ${mode === 'learning' ? 'learning' : ''}`} disabled={!selectedConnected || mode === 'recording'} onClick={() => mode === 'learning' ? void cancelCapture('Learn canceled.') : void learn()}>
            {mode === 'learning' ? '◉ Waiting for MIDI input…' : '＋ Learn MIDI Control'}
          </button>
          <div className="mapping-items">
            {mappings.length ? mappings.map(m => <button key={controlKey(m.control)} onClick={() => chooseMapping(m)} disabled={mode !== 'idle'} className={`mapping-item ${selected && controlKey(selected) === controlKey(m.control) ? 'active' : ''}`}>
              <span className="pad-glyph">▦</span><span className="mapping-copy"><strong>{m.label}</strong><small>{controlText(m.control)}</small></span><span className="item-chevron">›</span>
            </button>) : <div className="empty-list"><div>▥</div><strong>No controls mapped yet</strong><p>Press Learn MIDI Control, then press your hardware button.</p></div>}
          </div>
          {lastMessage && <div className="monitor"><span>LAST MIDI INPUT</span><strong>{controlText(lastMessage.control)}</strong><small>Value {lastMessage.value} · {lastMessage.is_press ? 'Pressed / high' : 'Released / low'}</small></div>}
        </section>
        <section className="panel editor">
          <div className="panel-heading"><div><h2>{selected ? controlText(selected) : 'Keyboard mapping'}</h2><p className="helper">{selected ? 'Assign recorded keys to this MIDI control.' : 'Learn a control first to create its mapping.'}</p></div>{existing && mode === 'idle' && <button className="text-danger" onClick={() => void clearMapping()}>Delete mapping</button>}</div>
          <label className="field-label" htmlFor="mapping-name">MAPPING NAME</label>
          <input id="mapping-name" className="text-input" maxLength={80} value={label} disabled={!selected || mode !== 'idle'} onChange={e => setLabel(e.target.value)} />
          <div className="field-label capture-title">RECORDED KEYBOARD INPUT</div>
          <div className={`capture ${mode === 'recording' ? 'capture-live' : ''}`} ref={captureRef} tabIndex={-1}>
            <div className="capture-dot">{mode === 'recording' ? '●' : '⌨'}</div>
            <strong>{mode === 'recording' ? 'Recording keys…' : shown.length ? 'Recorded sequence' : 'No keys recorded'}</strong>
            {shown.length ? <div className="chord-list">{shown.map((chord, i) => <div className="chord-line" key={i}><span className="step-count">{i + 1}</span>{chord.keys.map((key, j) => <span className="keycap" key={`${key}-${j}`}>{formatKey(key)}</span>)}<label className="delay-field"><input type="number" min={0} max={5000} step={10} className="delay-input" aria-label={`Delay after step ${i + 1} in milliseconds`} disabled={mode === 'learning'} value={chord.delay_after_ms} onChange={e => setDelay(i, e.target.value)} />ms</label>{i < shown.length - 1 && <span className="sequence-arrow">→</span>}</div>)}</div> : <p>{mode === 'recording' ? 'Press your keyboard now.' : 'Click Record Keyboard Mapping to start capturing presses.'}</p>}
            <small>{mode === 'recording' ? 'Capturing keyboard strokes until you confirm or cancel' : `${shown.length} keyboard step${shown.length === 1 ? '' : 's'}`}</small>
          </div>
          {mode === 'recording' ? <div className="button-row"><button className="primary" disabled={!draft.length || busy} onClick={() => void confirmMapping()}>{busy ? 'Saving…' : '✓ Confirm Mapping'}</button><button className="secondary" disabled={!draft.length} onClick={() => { setDraft(s => s.slice(0, -1)); captureRef.current?.focus(); }}>Undo last</button><button className="secondary" onClick={() => void cancelCapture('Recording canceled; saved mapping unchanged.')}>Cancel</button></div>
            : <div className="button-row">
              <button className="primary record-start" disabled={!selected || mode === 'learning'} onClick={() => void beginRecording()}>● &nbsp; Record Keyboard Mapping</button>
              {dirty && <button className="secondary" disabled={busy} onClick={() => void saveChanges()}>{busy ? 'Saving…' : 'Save changes'}</button>}
            </div>}
          <div className="info"><strong>How recording works</strong><p>Record a shortcut such as ⌘ C, or multiple shortcuts in order. Select Confirm Mapping to save. The MIDI control will replay those steps in sequence with a short fixed delay.</p></div>
        </section>
      </div>
      <div className="statusbar"><span className="dot green" />{message}<span className="status-right">MIDI 1.0 Note + CC buttons · macOS</span></div>
    </main>
  </div>;
}
