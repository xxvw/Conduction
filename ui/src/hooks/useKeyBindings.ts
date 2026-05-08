import { useCallback, useEffect, useState } from "react";

import {
  DEFAULT_BINDINGS,
  type ShortcutAction,
  type ShortcutBinding,
} from "@/lib/keybindings";

const STORAGE_KEY = "conduction.keybindings.v1";

/** localStorage から起動時に1回だけ読み込み、編集は localStorage に書き戻す。 */
export function useKeyBindings() {
  const [bindings, setBindings] = useState<ShortcutBinding[]>(loadOrDefault);

  useEffect(() => {
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(bindings));
    } catch {
      // ストレージ枯渇時等は無視
    }
  }, [bindings]);

  const setBinding = useCallback((action: ShortcutAction, key: string) => {
    setBindings((current) =>
      current.map((b) => (b.action === action ? { ...b, key } : b)),
    );
  }, []);

  const reset = useCallback(() => {
    setBindings(DEFAULT_BINDINGS);
  }, []);

  return { bindings, setBinding, reset };
}

function loadOrDefault(): ShortcutBinding[] {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return [...DEFAULT_BINDINGS];
    const parsed = JSON.parse(raw) as Partial<ShortcutBinding>[];
    if (!Array.isArray(parsed)) return [...DEFAULT_BINDINGS];
    // 全アクションを保証する：保存済みに無い action はデフォルトを補完。
    return DEFAULT_BINDINGS.map((d) => {
      const found = parsed.find((p) => p.action === d.action);
      if (found && typeof found.key === "string" && found.key.length > 0) {
        return { ...d, key: found.key };
      }
      return d;
    });
  } catch {
    return [...DEFAULT_BINDINGS];
  }
}
