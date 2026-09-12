import { useEffect, useState } from "react";

import { ipc, type TemplatePreset } from "@/lib/ipc";

interface TemplateLauncherProps {
  presets: TemplatePreset[];
  /** 起動時に渡す BPM (アクティブデッキの effective BPM)。0 以下なら disabled。 */
  currentBpm: number;
  onStart: (presetId: string, bpm: number, reverse: boolean) => void;
  externalMixer?: boolean;
}

interface CompatibilityCheck {
  compatible: boolean;
  error?: string;
}

export function TemplateLauncher({
  presets,
  currentBpm,
  onStart,
  externalMixer = false,
}: TemplateLauncherProps) {
  const [selected, setSelected] = useState<string>(presets[0]?.id ?? "");
  const [reverse, setReverse] = useState<boolean>(false);
  const [checks, setChecks] = useState<{
    presets: TemplatePreset[];
    results: Record<string, CompatibilityCheck>;
  } | null>(null);

  useEffect(() => {
    if (!externalMixer) {
      setChecks(null);
      return;
    }
    let cancelled = false;
    setChecks(null);
    void Promise.allSettled(presets.map(async (preset) => {
      const template = await ipc.getTemplatePreset(preset.id);
      if (!Array.isArray(template.tracks)) throw new Error("テンプレートの操作を読み取れませんでした。");
      return template.tracks.every(({ target }) =>
        target.type === "deck_echo_wet" || target.type === "deck_reverb_wet",
      );
    })).then((results) => {
      if (cancelled) return;
      const compatibility: Record<string, CompatibilityCheck> = {};
      results.forEach((result, index) => {
        const preset = presets[index];
        if (!preset) return;
        compatibility[preset.id] = result.status === "fulfilled"
          ? { compatible: result.value }
          : { compatible: false, error: String(result.reason) };
      });
      setChecks({ presets, results: compatibility });
    });
    return () => { cancelled = true; };
  }, [presets, externalMixer]);

  const checking = externalMixer && checks?.presets !== presets;
  const compatible = (preset: TemplatePreset) => !externalMixer
    || (!checking && checks?.results[preset.id]?.compatible === true);
  const available = presets.filter(compatible);
  const activeSelection = available.some((preset) => preset.id === selected)
    ? selected
    : available[0]?.id ?? "";

  useEffect(() => {
    if (checking) return;
    if (activeSelection) setSelected(activeSelection);
    else if (!presets.some((preset) => preset.id === selected)) setSelected("");
  }, [activeSelection, checking, presets, selected]);

  if (presets.length === 0) return null;

  const canStart = Number.isFinite(currentBpm) && currentBpm > 0 && activeSelection !== "";
  const failed = externalMixer && !checking
    ? presets.filter((preset) => checks?.results[preset.id]?.error)
    : [];
  const firstFailure = failed[0];
  const hint = !externalMixer ? "" : checking ? "External 互換性を確認中…"
    : firstFailure ? `${firstFailure.name}: ${checks?.results[firstFailure.id]?.error}${failed.length > 1 ? ` (ほか ${failed.length - 1} 件)` : ""}`
    : available.length === 0 ? "External 対応の FX テンプレートがありません。"
    : "External: Echo / Reverb テンプレートのみ使用できます。";

  return (
    <div className="transport-launcher" style={externalMixer ? { flexWrap: "wrap", flexShrink: 1, minWidth: 0, maxWidth: 560 } : undefined}>
      <span className="transport-launcher-label">TEMPLATE</span>
      <select
        className="transport-launcher-select"
        aria-label="トランジションテンプレート"
        style={externalMixer ? { maxWidth: 280, minWidth: 0 } : undefined}
        value={activeSelection}
        disabled={checking || available.length === 0}
        onChange={(e) => {
          if (available.some((preset) => preset.id === e.target.value)) setSelected(e.target.value);
        }}
      >
        {activeSelection === "" && <option value="" disabled>{checking ? "互換性を確認中…" : "使用できるテンプレートなし"}</option>}
        {presets.map((p) => (
          <option key={p.id} value={p.id} disabled={!compatible(p)}>
            {p.name} · {p.duration_beats}b{externalMixer && !checking && !compatible(p) ? checks?.results[p.id]?.error ? " · 確認失敗" : " · External 非対応" : ""}
          </option>
        ))}
      </select>
      <label
        className="transport-launcher-reverse"
        title="Reverse: deck A↔B swap + crossfader 符号反転 で起動"
      >
        <input
          type="checkbox"
          checked={reverse}
          onChange={(e) => setReverse(e.target.checked)}
        />
        <span>B→A</span>
      </label>
      <button
        type="button"
        className="transport-launcher-start"
        disabled={!canStart}
        onClick={() => {
          if (canStart) onStart(activeSelection, currentBpm, reverse);
        }}
        title={
          canStart
            ? `Start at ${currentBpm.toFixed(1)} BPM${reverse ? " (B→A)" : ""}`
            : activeSelection === "" ? hint : "Load a track on the active deck first"
        }
      >
        ▶ Start
      </button>
      {hint && <small className="transport-launcher-hint" style={{ flexBasis: "100%", fontSize: 10, overflowWrap: "anywhere" }} role={firstFailure ? "alert" : "status"}>{hint}</small>}
    </div>
  );
}
