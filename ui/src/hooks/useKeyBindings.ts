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
          const merged = mergeWithDefaults(settings.keybindings);
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

/**
 * 保存済み bindings を DEFAULT_BINDINGS に重ねる。
 *
 * 過去のスキーマ（例: 旧 'focus-deck-a' に "1" が割り当てられていた状態）を
 * 引きずって新しい DEFAULT_BINDINGS と key が衝突しないよう、結果に重複が
 * 残ったらデフォルトと違うエントリ側をデフォルトに戻す。これによって設定
 * ファイルが古いまま残っても新規アクションの初期キー（hotcue-1=1 など）が
 * 黙って奪われない。
 */
function mergeWithDefaults(
  saved: { action: string; key: string }[],
): ShortcutBinding[] {
  const merged = DEFAULT_BINDINGS.map((d) => {
    const found = saved.find((p) => p.action === d.action);
    if (found && typeof found.key === "string" && found.key.length > 0) {
      return { ...d, key: found.key };
    }
    return d;
  });

  // key の重複を数える（case insensitive）
  const norm = (k: string) => (k.length === 1 ? k.toLowerCase() : k);
  const counts = new Map<string, number>();
  for (const b of merged) {
    const k = norm(b.key);
    counts.set(k, (counts.get(k) ?? 0) + 1);
  }

  // 重複している binding のうち、デフォルトと違う key を持つものを default に戻す。
  // デフォルトと一致する側は守られる。
  return merged.map((b, i) => {
    const k = norm(b.key);
    if ((counts.get(k) ?? 0) > 1 && b.key !== DEFAULT_BINDINGS[i]!.key) {
      return { ...b, key: DEFAULT_BINDINGS[i]!.key };
    }
    return b;
  });
}
