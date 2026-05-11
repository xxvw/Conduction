import { useEffect, useState } from "react";

import { AutomationTimeline } from "@/components/templates/AutomationTimeline";
import { ipc, type TemplateFull, type TemplatePreset } from "@/lib/ipc";

export function TemplatesScreen() {
  const [presets, setPresets] = useState<TemplatePreset[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<TemplateFull | null>(null);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    ipc
      .listTemplatePresets()
      .then((p) => {
        if (cancelled) return;
        setPresets(p);
        if (p.length > 0) setSelectedId(p[0]!.id);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!selectedId) {
      setDetail(null);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    ipc
      .getTemplatePreset(selectedId)
      .then((t) => {
        if (!cancelled) setDetail(t);
      })
      .catch((e) => {
        if (!cancelled) {
          setError(String(e));
          setDetail(null);
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [selectedId]);

  return (
    <section className="templates-screen">
      <header className="templates-header">
        <h2>Templates</h2>
        <p className="hint">
          内蔵プリセットの自動化曲線を確認 (read-only)。編集 UI は次フェーズで実装。
        </p>
      </header>

      <div className="templates-layout">
        <aside className="templates-list">
          {presets.map((p) => (
            <button
              key={p.id}
              type="button"
              className="templates-list-item"
              data-active={p.id === selectedId}
              onClick={() => setSelectedId(p.id)}
            >
              <span className="templates-list-name">{p.name}</span>
              <span className="templates-list-meta">
                {p.duration_beats} beats · {(p.duration_beats / 4).toFixed(0)} bars
              </span>
            </button>
          ))}
        </aside>

        <div className="templates-editor">
          <header className="templates-editor-head">
            <h3>
              {detail?.name ?? "—"}{" "}
              {detail && (
                <span className="templates-editor-meta">
                  · {detail.tracks.length} track
                  {detail.tracks.length === 1 ? "" : "s"} ·{" "}
                  {detail.duration_beats} beats
                </span>
              )}
            </h3>
            <div className="templates-editor-mode-tabs">
              <button className="templates-editor-mode" data-active>
                Visual
              </button>
              <button className="templates-editor-mode" disabled title="Phase 4 残">
                Node
              </button>
              <button className="templates-editor-mode" disabled title="Phase 4 残">
                Script
              </button>
            </div>
          </header>

          {error && (
            <p className="hint" style={{ color: "var(--c-danger)" }}>
              {error}
            </p>
          )}
          {loading && <p className="hint">Loading…</p>}
          {detail && !loading && (
            <div className="templates-editor-body">
              <AutomationTimeline template={detail} />
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
