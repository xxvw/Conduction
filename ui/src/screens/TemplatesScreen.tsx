import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { AutomationTimeline } from "@/components/templates/AutomationTimeline";
import { NodeGraphEditor } from "@/components/templates/NodeGraphEditor";
import {
  ipc,
  type AutomationTrack,
  type TemplateFull,
  type TemplatePreset,
} from "@/lib/ipc";

type EditorMode = "visual" | "node";

export function TemplatesScreen() {
  const [presets, setPresets] = useState<TemplatePreset[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<TemplateFull | null>(null);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [nameDraft, setNameDraft] = useState<string>("");
  const [saving, setSaving] = useState<boolean>(false);
  // 編集中の tracks。null なら未編集。
  const [draftTracks, setDraftTracks] = useState<AutomationTrack[] | null>(null);
  const [editorMode, setEditorMode] = useState<EditorMode>("visual");
  // Node エディタで選択中の track index (Visual に keyframes を表示する時に使う)。
  const [selectedTrackIdx, setSelectedTrackIdx] = useState<number | null>(null);

  // async ハンドラから「現在の選択 ID」を読むための ref。
  // closure に閉じ込めた selectedId は古くなるので、await 後の判定には ref を使う。
  const selectedIdRef = useRef<string | null>(selectedId);
  useEffect(() => {
    selectedIdRef.current = selectedId;
  }, [selectedId]);

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
        setDraftTracks(null); // 別 template に切替時は draft を捨てる
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
    const sourceId = detail.id;
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
      // Duplicate 起動時の選択から変わっていなければ新しい dup へ遷移、
      // 変わっていれば現在の選択を尊重して refresh だけする。
      const stillOnSource = selectedIdRef.current === sourceId;
      await refreshPresets(stillOnSource ? saved.id : undefined);
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
    const targetId = detail.id;
    setSaving(true);
    try {
      const updated: TemplateFull = { ...detail, name: trimmed };
      const saved = await ipc.saveUserTemplate(updated);
      // 競合チェック: save 中にユーザーが別 template に切り替えていたら
      // detail / selection を上書きしない (refresh だけ走らせる)
      const stillSelected = selectedIdRef.current === targetId;
      if (stillSelected) {
        setDetail(saved);
      }
      await refreshPresets(stillSelected ? saved.id : undefined);
    } catch (e) {
      setError(String(e));
      setNameDraft(detail.name);
    } finally {
      setSaving(false);
    }
  }, [detail, isUserSelected, nameDraft, refreshPresets]);

  const handleSaveDraft = useCallback(async () => {
    if (!detail || !isUserSelected || !draftTracks) return;
    const targetId = detail.id;
    setSaving(true);
    try {
      const updated: TemplateFull = { ...detail, tracks: draftTracks };
      const saved = await ipc.saveUserTemplate(updated);
      const stillSelected = selectedIdRef.current === targetId;
      if (stillSelected) {
        setDetail(saved);
        setDraftTracks(null);
      }
      await refreshPresets(stillSelected ? saved.id : undefined);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }, [detail, isUserSelected, draftTracks, refreshPresets]);

  const handleDiscardDraft = useCallback(() => {
    setDraftTracks(null);
  }, []);

  const handleDelete = useCallback(async () => {
    if (!detail || !isUserSelected) return;
    if (!window.confirm(`Delete "${detail.name}"?`)) return;
    const targetId = detail.id;
    setSaving(true);
    try {
      await ipc.deleteUserTemplate(targetId);
      // 削除中にユーザーが別の preset に切り替えていたら、その選択を尊重する。
      if (selectedIdRef.current === targetId) {
        setSelectedId(null);
      }
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
                onClick={() => {
                  if (
                    draftTracks &&
                    !window.confirm("Discard unsaved changes?")
                  ) {
                    return;
                  }
                  setSelectedId(p.id);
                }}
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
                onClick={() => {
                  if (
                    draftTracks &&
                    !window.confirm("Discard unsaved changes?")
                  ) {
                    return;
                  }
                  setSelectedId(p.id);
                }}
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
              {detail && isUserSelected && draftTracks && (
                <>
                  <span
                    className="hint"
                    style={{ color: "var(--c-accent)", fontSize: "var(--fs-micro)" }}
                  >
                    unsaved
                  </span>
                  <button
                    className="btn"
                    onClick={handleDiscardDraft}
                    disabled={saving}
                    title="Revert to last saved state"
                  >
                    Discard
                  </button>
                  <button
                    className="btn"
                    data-variant="primary"
                    onClick={() => void handleSaveDraft()}
                    disabled={saving}
                  >
                    Save
                  </button>
                </>
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
              <button
                className="templates-editor-mode"
                data-active={editorMode === "visual" || undefined}
                onClick={() => setEditorMode("visual")}
              >
                Visual
              </button>
              <button
                className="templates-editor-mode"
                data-active={editorMode === "node" || undefined}
                onClick={() => setEditorMode("node")}
              >
                Node
              </button>
              <button
                className="templates-editor-mode"
                disabled
                title="Phase D4 (next)"
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
              {editorMode === "visual" ? (
                <>
                  <AutomationTimeline
                    template={
                      draftTracks
                        ? { ...detail, tracks: draftTracks }
                        : detail
                    }
                    editable={isUserSelected}
                    onTracksChange={(next) => setDraftTracks(next)}
                  />
                  {isUserSelected && (
                    <p
                      className="hint"
                      style={{
                        fontSize: "var(--fs-micro)",
                        marginTop: "var(--s-2)",
                      }}
                    >
                      Drag a keyframe to move (snaps to 1/4 beat). Double-click
                      empty area to add. Right-click to delete (each row keeps
                      at least 1).
                    </p>
                  )}
                </>
              ) : (
                <NodeGraphEditorPanel
                  detail={detail}
                  draftTracks={draftTracks}
                  editable={isUserSelected}
                  selectedTrackIdx={selectedTrackIdx}
                  onSelectTrack={setSelectedTrackIdx}
                  onTracksChange={(next) => setDraftTracks(next)}
                />
              )}
            </div>
          )}
        </div>
      </div>
    </section>
  );
}

/** Node タブの中身。グラフ + 選択中 source の keyframes を AutomationTimeline で表示。 */
function NodeGraphEditorPanel({
  detail,
  draftTracks,
  editable,
  selectedTrackIdx,
  onSelectTrack,
  onTracksChange,
}: {
  detail: TemplateFull;
  draftTracks: AutomationTrack[] | null;
  editable: boolean;
  selectedTrackIdx: number | null;
  onSelectTrack: (idx: number | null) => void;
  onTracksChange: (next: AutomationTrack[]) => void;
}) {
  const tracks = draftTracks ?? detail.tracks;
  const effective: TemplateFull = useMemo(
    () => ({ ...detail, tracks }),
    [detail, tracks],
  );
  // 選択中 track が範囲外になったら解除 (track 削除や preset 切替時)
  useEffect(() => {
    if (selectedTrackIdx != null && selectedTrackIdx >= tracks.length) {
      onSelectTrack(null);
    }
  }, [tracks.length, selectedTrackIdx, onSelectTrack]);

  const selectedTrackTemplate: TemplateFull | null =
    selectedTrackIdx != null && tracks[selectedTrackIdx]
      ? {
          ...detail,
          tracks: [tracks[selectedTrackIdx]!],
        }
      : null;

  const handleSelectedKeyframesChange = useCallback(
    (nextOneTrack: AutomationTrack[]) => {
      if (selectedTrackIdx == null) return;
      const nextOne = nextOneTrack[0];
      if (!nextOne) return;
      const merged = tracks.map((tr, i) =>
        i === selectedTrackIdx ? nextOne : tr,
      );
      onTracksChange(merged);
    },
    [selectedTrackIdx, tracks, onTracksChange],
  );

  return (
    <div className="node-editor-panel">
      <NodeGraphEditor
        template={effective}
        editable={editable}
        onTracksChange={onTracksChange}
        selectedTrackIdx={selectedTrackIdx}
        onSelectTrack={onSelectTrack}
      />
      <div className="node-editor-detail">
        {selectedTrackTemplate ? (
          <>
            <div className="node-editor-detail-head">
              Keyframes —{" "}
              <strong>
                {targetShortLabel(selectedTrackTemplate.tracks[0]!.target)}
              </strong>
            </div>
            <AutomationTimeline
              template={selectedTrackTemplate}
              editable={editable}
              onTracksChange={handleSelectedKeyframesChange}
            />
          </>
        ) : (
          <p className="hint" style={{ fontSize: "var(--fs-micro)" }}>
            Select a node above to edit its keyframes.
            {editable && tracks.length === 0 && " Add Track to start."}
          </p>
        )}
      </div>
    </div>
  );
}

function targetShortLabel(target: import("@/lib/ipc").BuiltInTarget): string {
  switch (target.type) {
    case "crossfader":
      return "Crossfader";
    case "master_volume":
      return "Master Vol";
    case "deck_volume":
      return `Deck ${target.deck} · Volume`;
    case "deck_eq_low":
      return `Deck ${target.deck} · EQ Low`;
    case "deck_eq_mid":
      return `Deck ${target.deck} · EQ Mid`;
    case "deck_eq_high":
      return `Deck ${target.deck} · EQ High`;
    case "deck_filter":
      return `Deck ${target.deck} · Filter`;
    case "deck_echo_wet":
      return `Deck ${target.deck} · Echo Wet`;
    case "deck_reverb_wet":
      return `Deck ${target.deck} · Reverb Wet`;
  }
}
