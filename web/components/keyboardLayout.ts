export interface KeyDef {
  label: string;
  sub?: string;
  width?: number;
  code?: string;
  modifier?: boolean;
}

export type KeyboardRow = KeyDef[];

export const KEYBOARD_ROWS: KeyboardRow[] = [
  [
    { label: "esc", width: 1.5, modifier: true, code: "Escape" },
    { label: "F1" },
    { label: "F2" },
    { label: "F3" },
    { label: "F4" },
    { label: "F5" },
    { label: "F6" },
    { label: "F7" },
    { label: "F8" },
    { label: "F9" },
    { label: "F10" },
    { label: "F11" },
    { label: "F12" },
    { label: "\u23cf", width: 1.5 },
  ],
  [
    { label: "`", sub: "~" },
    { label: "1", sub: "!" },
    { label: "2", sub: "@" },
    { label: "3", sub: "#" },
    { label: "4", sub: "$" },
    { label: "5", sub: "%" },
    { label: "6", sub: "^" },
    { label: "7", sub: "&" },
    { label: "8", sub: "*" },
    { label: "9", sub: "(" },
    { label: "0", sub: ")" },
    { label: "-", sub: "_" },
    { label: "=", sub: "+" },
    { label: "delete", width: 1.5, modifier: true, code: "Backspace" },
  ],
  [
    { label: "tab", width: 1.5, modifier: true, code: "Tab" },
    { label: "Q" },
    { label: "W" },
    { label: "E" },
    { label: "R" },
    { label: "T" },
    { label: "Y" },
    { label: "U" },
    { label: "I" },
    { label: "O" },
    { label: "P" },
    { label: "[", sub: "{" },
    { label: "]", sub: "}" },
    { label: "\\", sub: "|" },
  ],
  [
    { label: "caps lock", width: 1.8, modifier: true, code: "CapsLock" },
    { label: "A" },
    { label: "S", code: "KeyS" },
    { label: "D" },
    { label: "F" },
    { label: "G" },
    { label: "H" },
    { label: "J" },
    { label: "K" },
    { label: "L" },
    { label: ";", sub: ":" },
    { label: "'", sub: '"' },
    { label: "return", width: 1.8, modifier: true, code: "Enter" },
  ],
  [
    { label: "shift", width: 2.4, modifier: true, code: "ShiftLeft" },
    { label: "Z" },
    { label: "X" },
    { label: "C" },
    { label: "V" },
    { label: "B" },
    { label: "N" },
    { label: "M" },
    { label: ",", sub: "<" },
    { label: ".", sub: ">" },
    { label: "/", sub: "?" },
    { label: "shift", width: 2.4, modifier: true, code: "ShiftRight" },
  ],
  [
    { label: "fn", modifier: true },
    { label: "control", modifier: true, code: "ControlLeft" },
    { label: "option", modifier: true, code: "AltLeft" },
    { label: "command", width: 1.3, modifier: true, code: "MetaLeft" },
    { label: "", width: 5.4, code: "Space" },
    { label: "command", width: 1.3, modifier: true, code: "MetaRight" },
    { label: "option", modifier: true, code: "AltRight" },
  ],
];

export const HOTKEY_CODES = new Set(["MetaLeft", "MetaRight", "ShiftLeft", "ShiftRight", "KeyS"]);
