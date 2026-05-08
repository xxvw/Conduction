import { useEffect } from "react";

import { DEFAULT_BINDINGS, type ShortcutAction, type ShortcutBinding } from "@/lib/keybindings";

interface UseShortcutsArgs {
  bindings?: ShortcutBinding[];
  onAction: (action: ShortcutAction, e: KeyboardEvent) => void;
}

/**
 * グローバルキーボードショートカットを購読する。
 * input / textarea / contentEditable にフォーカスがある時は無視する。
 */
export function useShortcuts({ bindings = DEFAULT_BINDINGS, onAction }: UseShortcutsArgs) {
  useEffect(() => {
    function handle(e: KeyboardEvent) {
      const tag = (e.target as HTMLElement | null)?.tagName?.toLowerCase();
      if (tag === "input" || tag === "textarea" || (e.target as HTMLElement)?.isContentEditable) {
        return;
      }
      // case-insensitive 比較（ArrowLeft などはそのまま）
      const matched = bindings.find((b) =>
        b.key.length === 1 ? b.key.toLowerCase() === e.key.toLowerCase() : b.key === e.key,
      );
      if (!matched) return;
      e.preventDefault();
      onAction(matched.action, e);
    }
    window.addEventListener("keydown", handle);
    return () => window.removeEventListener("keydown", handle);
  }, [bindings, onAction]);
}
