import { useEffect, useMemo, useRef, useState } from "react";

import {
  displayKey,
  type ShortcutAction,
  type ShortcutBinding,
} from "@/lib/keybindings";

interface SettingsScreenProps {
  bindings: ShortcutBinding[];
  setBinding: (action: ShortcutAction, key: string) => void;
  reset: () => void;
}

export function SettingsScreen({ bindings, setBinding, reset }: SettingsScreenProps) {
  // 重複検出: 各 key が何回出現するか
  const duplicateKeys = useMemo(() => {
    const counts = new Map<string, number>();
    for (const b of bindings) {
      counts.set(b.key, (counts.get(b.key) ?? 0) + 1);
    }
    return new Set(Array.from(counts.entries()).filter(([, n]) => n > 1).map(([k]) => k));
  }, [bindings]);

  return (
    <section className="settings-screen">
      <header className="settings-header">
        <h2>Settings</h2>
        <p className="settings-subtitle">Conductionの操作をカスタマイズ.</p>
      </header>

      <section className="settings-section">
        <div className="settings-section-header">
          <h3>Keyboard shortcuts</h3>
          <button className="btn settings-reset" onClick={reset}>
            Reset to defaults
          </button>
        </div>
        <p className="hint">行をクリックして新しいキーをキャプチャ。Escでキャンセル。</p>

        <div className="keybindings-table">
          {bindings.map((b) => (
            <KeybindingRow
              key={b.action}
              binding={b}
              isDuplicate={duplicateKeys.has(b.key)}
              onChange={(key) => setBinding(b.action, key)}
            />
          ))}
        </div>
      </section>

      <section className="settings-section">
        <h3>About</h3>
        <p className="hint">
          設定は <code>localStorage</code> に保存されます。クリアしたい場合は Reset を押してください。
        </p>
      </section>
    </section>
  );
}

function KeybindingRow({
  binding,
  isDuplicate,
  onChange,
}: {
  binding: ShortcutBinding;
  isDuplicate: boolean;
  onChange: (key: string) => void;
}) {
  const [capturing, setCapturing] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!capturing) return;
    const handler = (e: KeyboardEvent) => {
      // capture phase で吸い込んで他のグローバル listener に行かないようにする
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        setCapturing(false);
        return;
      }
      // Modifier キー単独の入力（Shift等）はスキップ。
      if (["Shift", "Control", "Alt", "Meta"].includes(e.key)) return;
      onChange(e.key);
      setCapturing(false);
    };
    window.addEventListener("keydown", handler, true);
    return () => window.removeEventListener("keydown", handler, true);
  }, [capturing, onChange]);

  return (
    <div className="keybinding-row" data-duplicate={isDuplicate || undefined}>
      <span className="keybinding-action">{binding.label}</span>
      <button
        ref={buttonRef}
        className="keybinding-button"
        data-capturing={capturing || undefined}
        onClick={() => setCapturing(true)}
      >
        {capturing ? "Press a key…" : displayKey(binding.key)}
      </button>
      {isDuplicate && <span className="keybinding-warn">duplicate</span>}
    </div>
  );
}
