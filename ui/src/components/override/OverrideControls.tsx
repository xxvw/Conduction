import { ipc } from "@/lib/ipc";
import type { AutomationModeKind } from "@/types/mixer";

interface OverrideControlsProps {
  targetKey: string;
  /** 現在の AutomationMode。未実行 (=Idle) なら全ボタン無効。 */
  mode: AutomationModeKind;
  templateActive: boolean;
}

export function OverrideControls({
  targetKey,
  mode,
  templateActive,
}: OverrideControlsProps) {
  const canOverride =
    templateActive && (mode === "automated" || mode === "resuming");
  const canResume =
    templateActive && (mode === "overridden" || mode === "committed");
  const canCommit =
    templateActive && (mode === "overridden" || mode === "resuming");

  return (
    <div className="override-controls" data-mode={mode}>
      <span className="override-dot" data-mode={mode} title={modeLabel(mode)} />
      <button
        type="button"
        className="override-btn"
        disabled={!canOverride}
        onClick={() => void ipc.overrideParam(targetKey)}
        title="O — Override (take manual control)"
      >
        OVR
      </button>
      <button
        type="button"
        className="override-btn"
        disabled={!canResume}
        onClick={() => void ipc.resumeParam(targetKey, 4)}
        title="R — Resume (Glide back to automation over 4 beats)"
      >
        R
      </button>
      <button
        type="button"
        className="override-btn"
        disabled={!canCommit}
        onClick={() => void ipc.commitParam(targetKey)}
        title="C — Commit (lock manual value)"
      >
        C
      </button>
    </div>
  );
}

function modeLabel(mode: AutomationModeKind): string {
  switch (mode) {
    case "idle":
      return "Idle (no template)";
    case "automated":
      return "Automated (template controls)";
    case "overridden":
      return "Overridden (manual)";
    case "resuming":
      return "Resuming (gliding back)";
    case "committed":
      return "Committed (manual locked)";
  }
}
