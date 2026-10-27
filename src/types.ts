export type ControlKind = 'note' | 'cc';
export interface MidiControl { kind: ControlKind; channel: number; number: number }
export interface KeyChord { keys: string[]; delay_after_ms: number }
export interface Mapping { device_id: string; control: MidiControl; label: string; steps: KeyChord[] }
export interface StoredData { schema_version: number; selected_device_id: string | null; mappings: Mapping[] }
export interface MidiDevice { id: string; name: string }
export interface MidiEvent { device_id: string; control: MidiControl; value: number; is_press: boolean }
export const controlKey = (c: MidiControl) => `${c.kind}:${c.channel}:${c.number}`;
export const MODIFIERS = ['Meta', 'Control', 'Alt', 'Shift'];
export const isModifier = (s: string) => MODIFIERS.includes(s);
export const supportedKey = (code: string) => /^(Key[A-Z]|Digit[0-9]|F([1-9]|1[0-2]))$/.test(code) || [
  'Space','Enter','NumpadEnter','Tab','Escape','Backspace','Delete',
  'ArrowUp','ArrowDown','ArrowLeft','ArrowRight','Home','End','PageUp','PageDown',
  'Minus','Equal','BracketLeft','BracketRight','Backslash','Semicolon','Quote',
  'Comma','Period','Slash','Backquote'
].includes(code);
export function formatKey(k: string) {
  return ({Meta:'⌘', Control:'⌃', Alt:'⌥', Shift:'⇧', Space:'Space', Enter:'↵',
    ArrowUp:'↑', ArrowDown:'↓', ArrowLeft:'←', ArrowRight:'→', Escape:'Esc',
    Backspace:'⌫', Delete:'Del'} as Record<string,string>)[k] ??
    (k.startsWith('Key') ? k.substring(3) : k.startsWith('Digit') ? k.substring(5) : k);
}
