import { useState } from "react";

import type { TemplatePreset } from "@/lib/ipc";

interface TemplateLauncherProps {
  presets: TemplatePreset[];
  /** 起動時に渡す BPM (アクティブデッキの effective BPM)。0 以下なら disabled。 */
  currentBpm: number;
  onStart: (presetId: string, bpm: number, reverse: boolean) => void;
}

export function TemplateLauncher({
  presets,
  currentBpm,
  onStart,
}: TemplateLauncherProps) {
  const [selected, setSelected] = useState<string>(presets[0]?.id ?? "");
  const [reverse, setReverse] = useState<boolean>(false);

  if (presets.length === 0) return null;

  const canStart = currentBpm > 0 && selected !== "";

  return (
    <div className="transport-launcher">
      <span className="transport-launcher-label">TEMPLATE</span>
      <select
        className="transport-launcher-select"
        value={selected}
        onChange={(e) => setSelected(e.target.value)}
      >
        {presets.map((p) => (
          <option key={p.id} value={p.id}>
            {p.name} · {p.duration_beats}b
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
          if (canStart) onStart(selected, currentBpm, reverse);
        }}
        title={
          canStart
            ? `Start at ${currentBpm.toFixed(1)} BPM${reverse ? " (B→A)" : ""}`
            : "Load a track on the active deck first"
        }
      >
        ▶ Start
      </button>
    </div>
  );
}
