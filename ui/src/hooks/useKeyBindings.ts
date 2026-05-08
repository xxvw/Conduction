import { useCallback, useEffect, useRef, useState } from "react";

import { ipc } from "@/lib/ipc";
import {
  DEFAULT_BINDINGS,
  type ShortcutAction,
  type ShortcutBinding,
} from "@/lib/keybindings";

/**
 * バックエンド (TOML on disk) からキーバインドを読み込み、変更時に保存する。
 *
 * - 起動時: 即座に DEFAULT_BINDINGS を返し、非同期で TOML から実値を取得して上書き。
 * - 変更時: state を即更新し、バックエンドに save_settings を投げる。
 *
 * 永続化先は要件 §13 の通り `~/Library/Application Support/com.xxvw.conduction/settings.toml`
 * （macOS）。
 */
export function useKeyBindings() {
  const [bindings, setBindings] = useState<ShortcutBinding[]>(() => [...DEFAULT_BINDINGS]);
  // ロード完了するまでは変更を save しない（初期 mount で空ファイルを書き戻すのを防ぐ）
  const loadedRef = useRef(false);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const settings = await ipc.getSettings();
        if (cancelled) return;
        if (settings.keybindings && settings.keybindings.length > 0) {
          // 保存された key で DEFAULT_BINDINGS を上書き（label は常にコード側を採用）
          const merged = DEFAULT_BINDINGS.map((d) => {
            const found = settings.keybindings.find((p) => p.action === d.action);
            if (found && typeof found.key === "string" && found.key.length > 0) {
              return { ...d, key: found.key };
            }
            return d;
          });
          setBindings(merged);
        }
      } catch (e) {
        console.warn("failed to load settings; using defaults:", e);
      } finally {
        loadedRef.current = true;
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const persist = useCallback((next: ShortcutBinding[]) => {
    if (!loadedRef.current) return;
    void ipc.saveSettings({
      keybindings: next.map((b) => ({
        action: b.action,
        key: b.key,
        label: b.label,
      })),
    });
  }, []);

  const setBinding = useCallback(
    (action: ShortcutAction, key: string) => {
      setBindings((current) => {
        const next = current.map((b) => (b.action === action ? { ...b, key } : b));
        persist(next);
        return next;
      });
    },
    [persist],
  );

  const reset = useCallback(() => {
    const next = [...DEFAULT_BINDINGS];
    setBindings(next);
    persist(next);
  }, [persist]);

  return { bindings, setBinding, reset };
}
