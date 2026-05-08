// 現状はハードコード。後段で設定画面から書き換えられるようにする（Phase 3b-10）。

export type ShortcutAction =
  | "seek-back-1"
  | "seek-fwd-1"
  | "seek-back-2"
  | "seek-fwd-2"
  | "seek-back-4"
  | "seek-fwd-4"
  | "focus-deck-a"
  | "focus-deck-b"
  | "zoom-in"
  | "zoom-out"
  | "play-pause"
  | "hotcue-1"
  | "hotcue-2"
  | "hotcue-3"
  | "hotcue-4"
  | "hotcue-5"
  | "hotcue-6"
  | "hotcue-7"
  | "hotcue-8"
  | "loop-in"
  | "loop-out"
  | "loop-toggle"
  | "loop-extend"
  | "loop-shrink";

export interface ShortcutBinding {
  key: string;
  action: ShortcutAction;
  /** 表示用ラベル */
  label: string;
}

/** key は KeyboardEvent.key と完全一致（caseはdedicate関数で吸収）。 */
export const DEFAULT_BINDINGS: ShortcutBinding[] = [
  { key: "ArrowLeft",  action: "seek-back-1", label: "1 beat back"  },
  { key: "ArrowRight", action: "seek-fwd-1",  label: "1 beat fwd"   },
  { key: "i",          action: "seek-back-2", label: "2 beats back" },
  { key: "o",          action: "seek-fwd-2",  label: "2 beats fwd"  },
  { key: "k",          action: "seek-back-4", label: "4 beats back" },
  { key: "l",          action: "seek-fwd-4",  label: "4 beats fwd"  },
  { key: "ArrowUp",    action: "zoom-in",     label: "zoom in"      },
  { key: "ArrowDown",  action: "zoom-out",    label: "zoom out"     },
  { key: "q",          action: "focus-deck-a", label: "focus Deck A" },
  { key: "w",          action: "focus-deck-b", label: "focus Deck B" },
  { key: " ",          action: "play-pause",   label: "play / pause" },
  { key: "[",          action: "loop-in",     label: "Loop In"      },
  { key: "]",          action: "loop-out",    label: "Loop Out"     },
  { key: "\\",         action: "loop-toggle", label: "Loop on/off"  },
  { key: ".",          action: "loop-extend", label: "Loop +1 bar"  },
  { key: ",",          action: "loop-shrink", label: "Loop −1 bar"  },
  { key: "1",          action: "hotcue-1",    label: "Hot Cue 1"     },
  { key: "2",          action: "hotcue-2",    label: "Hot Cue 2"     },
  { key: "3",          action: "hotcue-3",    label: "Hot Cue 3"     },
  { key: "4",          action: "hotcue-4",    label: "Hot Cue 4"     },
  { key: "5",          action: "hotcue-5",    label: "Hot Cue 5"     },
  { key: "6",          action: "hotcue-6",    label: "Hot Cue 6"     },
  { key: "7",          action: "hotcue-7",    label: "Hot Cue 7"     },
  { key: "8",          action: "hotcue-8",    label: "Hot Cue 8"     },
];

/** ズームレベル（秒）。要素は降順 (zoom-in 方向)。 */
export const ZOOM_LEVELS_SEC: readonly number[] = [1, 2, 4, 8, 16] as const;
export const DEFAULT_ZOOM_SEC = 4;

/** 表示用に "ArrowLeft" → "←" のような短縮ラベルへ。 */
export function displayKey(key: string): string {
  switch (key) {
    case "ArrowLeft":  return "←";
    case "ArrowRight": return "→";
    case "ArrowUp":    return "↑";
    case "ArrowDown":  return "↓";
    case " ":          return "Space";
    case "Escape":     return "Esc";
    default:           return key.length === 1 ? key.toUpperCase() : key;
  }
}
