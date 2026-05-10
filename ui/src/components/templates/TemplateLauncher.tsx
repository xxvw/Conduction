import { useState } from "react";

import type { TemplatePreset } from "@/lib/ipc";

interface TemplateLauncherProps {
  presets: TemplatePreset[];
  /** 起動時に渡す BPM (アクティブデッキの effective BPM)。0 以下なら disabled。 */
  currentBpm: number;
  onStart: (presetId: string, bpm: number) => void;
}

export function TemplateLauncher({
  presets,
  currentBpm,
  onStart,
}: TemplateLauncherProps) {
  const [selected, setSelected] = useState<string>(presets[0]?.id ?? "");

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
      <button
        type="button"
        className="transport-launcher-start"
        disabled={!canStart}
        onClick={() => {
          if (canStart) onStart(selected, currentBpm);
        }}
        title={
          canStart
            ? `Start at ${currentBpm.toFixed(1)} BPM`
            : "Load a track on the active deck first"
        }
      >
        ▶ Start
      </button>
    </div>
  );
}
