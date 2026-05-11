import { useCallback, useEffect, useMemo, useState } from "react";

import { AutomationTimeline } from "@/components/templates/AutomationTimeline";
import { ipc, type TemplateFull, type TemplatePreset } from "@/lib/ipc";

export function TemplatesScreen() {
  const [presets, setPresets] = useState<TemplatePreset[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<TemplateFull | null>(null);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [nameDraft, setNameDraft] = useState<string>("");
  const [saving, setSaving] = useState<boolean>(false);

  const refreshPresets = useCallback(async (keepId?: string | null) => {
    try {
      const list = await ipc.listTemplatePresets();
      setPresets(list);
      // 選択 ID の付け替え
      if (keepId && list.some((p) => p.id === keepId)) {
        setSelectedId(keepId);
      } else if (!selectedId || !list.some((p) => p.id === selectedId)) {
        setSelectedId(list[0]?.id ?? null);
      }
      return list;
    } catch (e) {
      setError(String(e));
      return [];
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    void refreshPresets();
  }, [refreshPresets]);

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
        if (cancelled) return;
        setDetail(t);
        setNameDraft(t.name);
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

  const { builtin, user } = useMemo(() => {
    const b: TemplatePreset[] = [];
    const u: TemplatePreset[] = [];
    for (const p of presets) {
      if (p.kind === "user") u.push(p);
      else b.push(p);
    }
    return { builtin: b, user: u };
  }, [presets]);

  const isUserSelected =
    detail !== null && detail.id.startsWith("user.");

  const handleDuplicate = useCallback(async () => {
    if (!detail) return;
    setSaving(true);
    try {
      const dup: TemplateFull = {
        // backend が空 id を user.<uuid> に置換するので空に
        id: "",
        name: `${detail.name} (copy)`,
        duration_beats: detail.duration_beats,
        tracks: detail.tracks,
      };
      const saved = await ipc.saveUserTemplate(dup);
      await refreshPresets(saved.id);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }, [detail, refreshPresets]);

  const handleRename = useCallback(async () => {
    if (!detail || !isUserSelected) return;
    const trimmed = nameDraft.trim();
    if (!trimmed || trimmed === detail.name) return;
    setSaving(true);
    try {
      const updated: TemplateFull = { ...detail, name: trimmed };
      const saved = await ipc.saveUserTemplate(updated);
      setDetail(saved);
      await refreshPresets(saved.id);
    } catch (e) {
      setError(String(e));
      setNameDraft(detail.name);
    } finally {
      setSaving(false);
    }
  }, [detail, isUserSelected, nameDraft, refreshPresets]);

  const handleDelete = useCallback(async () => {
    if (!detail || !isUserSelected) return;
    if (!window.confirm(`Delete "${detail.name}"?`)) return;
    setSaving(true);
    try {
      await ipc.deleteUserTemplate(detail.id);
      setSelectedId(null);
      await refreshPresets();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }, [detail, isUserSelected, refreshPresets]);

  return (
    <section className="templates-screen">
      <header className="templates-header">
        <h2>Templates</h2>
        <p className="hint">
          内蔵プリセットは read-only。Duplicate でユーザー側にコピーすれば
          rename / 削除 / TemplateLauncher で再生できる。Keyframe drag 編集は次フェーズで。
        </p>
      </header>

      <div className="templates-layout">
        <aside className="templates-list">
          <div className="templates-list-section">
            <div className="templates-list-section-label">Built-in</div>
            {builtin.map((p) => (
              <button
                key={p.id}
                type="button"
                className="templates-list-item"
                data-active={p.id === selectedId}
                onClick={() => setSelectedId(p.id)}
              >
                <span className="templates-list-name">{p.name}</span>
                <span className="templates-list-meta">
                  {p.duration_beats} beats ·{" "}
                  {(p.duration_beats / 4).toFixed(0)} bars
                </span>
              </button>
            ))}
          </div>

          <div className="templates-list-section">
            <div className="templates-list-section-label">User</div>
            {user.length === 0 && (
              <p className="hint" style={{ padding: "0 var(--s-2)" }}>
                No user templates. Select a built-in and click Duplicate.
              </p>
            )}
            {user.map((p) => (
              <button
                key={p.id}
                type="button"
                className="templates-list-item"
                data-active={p.id === selectedId}
                data-user
                onClick={() => setSelectedId(p.id)}
              >
                <span className="templates-list-name">{p.name}</span>
                <span className="templates-list-meta">
                  {p.duration_beats} beats ·{" "}
                  {(p.duration_beats / 4).toFixed(0)} bars
                </span>
              </button>
            ))}
          </div>
        </aside>

        <div className="templates-editor">
          <header className="templates-editor-head">
            <div className="templates-editor-title">
              {isUserSelected ? (
                <input
                  type="text"
                  className="templates-editor-name-input"
                  value={nameDraft}
                  disabled={saving}
                  onChange={(e) => setNameDraft(e.target.value)}
                  onBlur={() => void handleRename()}
                  onKeyDown={(e) => {
                    if (e.key === "Enter")
                      (e.target as HTMLInputElement).blur();
                    if (e.key === "Escape" && detail) {
                      setNameDraft(detail.name);
                      (e.target as HTMLInputElement).blur();
                    }
                  }}
                />
              ) : (
                <h3>{detail?.name ?? "—"}</h3>
              )}
              {detail && (
                <span className="templates-editor-meta">
                  {detail.tracks.length} track
                  {detail.tracks.length === 1 ? "" : "s"} ·{" "}
                  {detail.duration_beats} beats
                </span>
              )}
            </div>
            <div className="templates-editor-actions">
              {detail && !isUserSelected && (
                <button
                  className="btn"
                  onClick={handleDuplicate}
                  disabled={saving}
                  title="Copy this preset into editable user templates"
                >
                  Duplicate
                </button>
              )}
              {detail && isUserSelected && (
                <button
                  className="btn"
                  data-variant="danger"
                  onClick={handleDelete}
                  disabled={saving}
                >
                  Delete
                </button>
              )}
            </div>
            <div className="templates-editor-mode-tabs">
              <button className="templates-editor-mode" data-active>
                Visual
              </button>
              <button
                className="templates-editor-mode"
                disabled
                title="Phase D4"
              >
                Node
              </button>
              <button
                className="templates-editor-mode"
                disabled
                title="Phase D4"
              >
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
