import { useEffect, useMemo, useRef, useState } from "react";

import { useAudioSettings } from "@/hooks/useAudioSettings";
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

type SettingsTab = "general" | "api";

const HTTP_API_BASE = "http://127.0.0.1:38127";

export function SettingsScreen({ bindings, setBinding, reset }: SettingsScreenProps) {
  const [tab, setTab] = useState<SettingsTab>("general");

  return (
    <section className="settings-screen" data-tab={tab}>
      {tab !== "api" && (
        <header className="settings-header">
          <h2>Settings</h2>
          <p className="settings-subtitle">Conductionの操作をカスタマイズ.</p>
        </header>
      )}

      <div className="settings-layout">
        <aside className="settings-tabs" role="tablist" aria-label="Settings tabs">
          <button
            role="tab"
            className="settings-tab"
            data-active={tab === "general"}
            aria-selected={tab === "general"}
            onClick={() => setTab("general")}
          >
            General
          </button>
          <button
            role="tab"
            className="settings-tab"
            data-active={tab === "api"}
            aria-selected={tab === "api"}
            onClick={() => setTab("api")}
          >
            API Docs
          </button>
        </aside>

        <div className="settings-content" role="tabpanel">
          {tab === "general" && (
            <GeneralTab
              bindings={bindings}
              setBinding={setBinding}
              reset={reset}
            />
          )}
          {tab === "api" && <ApiDocsTab />}
        </div>
      </div>
    </section>
  );
}

function GeneralTab({ bindings, setBinding, reset }: SettingsScreenProps) {
  const duplicateKeys = useMemo(() => {
    const counts = new Map<string, number>();
    for (const b of bindings) {
      counts.set(b.key, (counts.get(b.key) ?? 0) + 1);
    }
    return new Set(Array.from(counts.entries()).filter(([, n]) => n > 1).map(([k]) => k));
  }, [bindings]);

  return (
    <>
      <AudioOutputSection />

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
          キーバインドと出力デバイスの設定は OS の設定ファイル
          (<code>~/Library/Application Support/com.xxvw.conduction/settings.toml</code>)
          に保存されます。
        </p>
      </section>
    </>
  );
}

function ApiDocsTab() {
  return (
    <div className="api-docs-fullscreen">
      <iframe
        title="Conduction API documentation"
        src={`${HTTP_API_BASE}/swagger-ui/`}
        className="api-docs-iframe-fullscreen"
      />
    </div>
  );
}

function AudioOutputSection() {
  const { state, setMain, setCue } = useAudioSettings();
  return (
    <section className="settings-section">
      <div className="settings-section-header">
        <h3>Audio output</h3>
      </div>
      <p className="hint">
        変更を反映するにはアプリを再起動してください。
      </p>
      <div className="audio-device-grid">
        <DeviceSelect
          label="MAIN"
          description="観客に流す主出力"
          value={state.mainOutput}
          devices={state.devices}
          loading={state.loading}
          allowNone={false}
          onChange={(v) => void setMain(v)}
        />
        <DeviceSelect
          label="CUE"
          description="ヘッドホン用モニタリング (PFL)"
          value={state.cueOutput}
          devices={state.devices}
          loading={state.loading}
          allowNone={true}
          onChange={(v) => void setCue(v)}
        />
      </div>
    </section>
  );
}

function DeviceSelect({
  label,
  description,
  value,
  devices,
  loading,
  allowNone,
  onChange,
}: {
  label: string;
  description: string;
  value: string | null;
  devices: string[];
  loading: boolean;
  allowNone: boolean;
  onChange: (v: string | null) => void;
}) {
  return (
    <div className="audio-device-row">
      <div className="audio-device-meta">
        <span className="audio-device-label">{label}</span>
        <span className="audio-device-desc">{description}</span>
      </div>
      <select
        className="audio-device-select"
        value={value ?? ""}
        disabled={loading || devices.length === 0}
        onChange={(e) => {
          const v = e.target.value;
          onChange(v === "" ? null : v);
        }}
      >
        {allowNone && <option value="">— Off (no Cue output) —</option>}
        {!allowNone && value == null && (
          <option value="">— System default —</option>
        )}
        {devices.map((d) => (
          <option key={d} value={d}>
            {d}
          </option>
        ))}
      </select>
    </div>
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
