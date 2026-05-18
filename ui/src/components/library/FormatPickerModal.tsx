import { useEffect, useState } from "react";

import "./FormatPickerModal.css";
import { ipc, type FormatInfo, type TargetKind } from "@/lib/ipc";

interface FormatPickerModalProps {
  mode: "export" | "import";
  open: boolean;
  onClose: () => void;
  onPick: (format: FormatInfo) => void;
}

export function FormatPickerModal({
  mode,
  open,
  onClose,
  onPick,
}: FormatPickerModalProps) {
  const [formats, setFormats] = useState<FormatInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setLoading(true);
    setError(null);
    const fetcher =
      mode === "export" ? ipc.listExportFormats() : ipc.listImportFormats();
    fetcher
      .then((list) => {
        if (!cancelled) setFormats(list);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [open, mode]);

  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [open, onClose]);

  if (!open) return null;
  const title = mode === "export" ? "Export library" : "Import library";

  return (
    <div className="format-picker-backdrop" onClick={onClose}>
      <div
        className="format-picker"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-label={title}
      >
        <header className="format-picker-head">
          <strong>{title}</strong>
          <button
            className="format-picker-close"
            type="button"
            aria-label="close"
            onClick={onClose}
          >
            ×
          </button>
        </header>

        {loading && <p className="hint">Loading formats…</p>}
        {error && (
          <p className="hint" style={{ color: "var(--c-danger)" }}>
            {error}
          </p>
        )}

        <ul className="format-picker-list">
          {formats.map((f) => (
            <li key={f.id}>
              <button
                type="button"
                className="format-picker-row"
                disabled={!f.available}
                onClick={() => f.available && onPick(f)}
                title={
                  f.available
                    ? `Pick ${f.label}`
                    : `${f.label} is not implemented in this build yet`
                }
              >
                <span className="format-picker-label">{f.label}</span>
                <span className="format-picker-target">
                  {targetKindLabel(f.target_kind)}
                </span>
                {!f.available && (
                  <span className="format-picker-badge">coming soon</span>
                )}
              </button>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}

function targetKindLabel(k: TargetKind): string {
  switch (k) {
    case "file":
      return "single file";
    case "directory":
      return "directory";
    case "in-place":
      return "tags in audio files";
  }
}
