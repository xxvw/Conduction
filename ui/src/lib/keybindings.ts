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
  | "play-pause";

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
  { key: "1",          action: "focus-deck-a", label: "focus Deck A" },
  { key: "2",          action: "focus-deck-b", label: "focus Deck B" },
  { key: " ",          action: "play-pause",   label: "play / pause" },
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
