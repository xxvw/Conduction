import type { TemplateStatus } from "@/types/mixer";

interface TransportStatusPanelProps {
  status: TemplateStatus | null;
  onAbort: () => void;
}

export function TransportStatusPanel({
  status,
  onAbort,
}: TransportStatusPanelProps) {
  if (!status) return null;

  const pct = Math.round(Math.max(0, Math.min(1, status.progress)) * 100);
  const remainingBars = Math.max(0, Math.ceil(status.beats_remaining / 4));

  return (
    <div className="transport-status">
      <div className="transport-status-row">
        <span className="transport-status-label">▶ TEMPLATE</span>
        <span className="transport-status-name">{status.name}</span>
        {status.override_count > 0 && (
          <span className="transport-status-overrides" title="Parameters under manual control">
            {status.override_count} override{status.override_count > 1 ? "s" : ""}
          </span>
        )}
        <span className="transport-status-progress-text">
          {pct}% · {remainingBars} bars left
        </span>
        <button
          type="button"
          className="transport-status-abort"
          onClick={() => {
            if (window.confirm("Abort template? Current parameters will hold.")) {
              onAbort();
            }
          }}
          title="Shift+Esc — abort template"
        >
          Abort
        </button>
      </div>
      <div className="transport-status-bar">
        <div
          className="transport-status-bar-fill"
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  );
}
