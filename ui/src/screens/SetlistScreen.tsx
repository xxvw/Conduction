import { useCallback, useEffect, useMemo, useState } from "react";

import {
  ipc,
  type SetlistDto,
  type SetlistEntryDto,
  type TempoMode,
  type TemplatePreset,
  type TransitionSpec,
} from "@/lib/ipc";
import type { DeckId } from "@/types/mixer";
import type { TrackSummary } from "@/types/track";

interface SetlistScreenProps {
  tracks: TrackSummary[];
  onLoadToDeck: (deck: DeckId, path: string) => void;
}

const TEMPO_MODE_LABELS: Record<TempoMode, string> = {
  hold_source: "Hold Source",
  match_target: "Match Target",
  linear_blend: "Linear Blend",
  master_tempo: "Master Tempo",
};

export function SetlistScreen({ tracks, onLoadToDeck }: SetlistScreenProps) {
  const [setlists, setSetlists] = useState<SetlistDto[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [presets, setPresets] = useState<TemplatePreset[]>([]);
  const [newName, setNewName] = useState("");

  const refresh = useCallback(async () => {
    try {
      const list = await ipc.listSetlists();
      setSetlists(list);
      setError(null);
      // 削除直後は selectedId が消えるので、先頭を選び直す
      setSelectedId((cur) => {
        if (cur && list.some((s) => s.id === cur)) return cur;
        return list[0]?.id ?? null;
      });
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    ipc
      .listTemplatePresets()
      .then(setPresets)
      .catch(() => {});
  }, []);

  const tracksById = useMemo(() => {
    const m = new Map<string, TrackSummary>();
    for (const t of tracks) m.set(t.id, t);
    return m;
  }, [tracks]);

  const selected = setlists.find((s) => s.id === selectedId) ?? null;

  const handleCreate = useCallback(async () => {
    const name = newName.trim();
    if (!name) return;
    try {
      const s = await ipc.createSetlist(name);
      setNewName("");
      setSelectedId(s.id);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }, [newName, refresh]);

  const handleDelete = useCallback(
    async (id: string) => {
      if (!window.confirm("Delete this setlist?")) return;
      try {
        await ipc.deleteSetlist(id);
        await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [refresh],
  );

  const handleRename = useCallback(
    async (id: string, name: string) => {
      const trimmed = name.trim();
      if (!trimmed) return;
      try {
        await ipc.renameSetlist(id, trimmed);
        await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [refresh],
  );

  const handleAddEntry = useCallback(
    async (trackId: string) => {
      if (!selectedId) return;
      try {
        await ipc.setlistAddEntry(selectedId, trackId);
        await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [selectedId, refresh],
  );

  const handleRemoveEntry = useCallback(
    async (entryId: string) => {
      if (!selectedId) return;
      try {
        await ipc.setlistRemoveEntry(selectedId, entryId);
        await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [selectedId, refresh],
  );

  const handleMoveEntry = useCallback(
    async (entryId: string, newIndex: number) => {
      if (!selectedId) return;
      try {
        await ipc.setlistMoveEntry(selectedId, entryId, newIndex);
        await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [selectedId, refresh],
  );

  const handleSetTransition = useCallback(
    async (entryId: string, spec: TransitionSpec | null) => {
      if (!selectedId) return;
      try {
        await ipc.setlistSetTransition(selectedId, entryId, spec);
        await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [selectedId, refresh],
  );

  return (
    <section className="setlist-screen">
      <aside className="setlist-list-panel">
        <header className="setlist-list-header">
          <strong>Setlists</strong>
        </header>
        <div className="setlist-create">
          <input
            type="text"
            placeholder="new setlist name"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void handleCreate();
            }}
          />
          <button
            className="btn"
            onClick={handleCreate}
            disabled={!newName.trim()}
          >
            + New
          </button>
        </div>
        {error && (
          <p className="hint" style={{ color: "var(--c-danger)" }}>
            {error}
          </p>
        )}
        <ul className="setlist-list">
          {setlists.map((s) => (
            <li
              key={s.id}
              className="setlist-list-item"
              data-active={s.id === selectedId}
              onClick={() => setSelectedId(s.id)}
            >
              <span className="setlist-list-name">{s.name}</span>
              <span className="setlist-list-count">{s.entries.length}</span>
              <button
                className="chip danger"
                title="Delete setlist"
                onClick={(e) => {
                  e.stopPropagation();
                  void handleDelete(s.id);
                }}
              >
                ×
              </button>
            </li>
          ))}
          {setlists.length === 0 && (
            <li className="hint">No setlists yet. Create one above.</li>
          )}
        </ul>
      </aside>

      <div className="setlist-detail-panel">
        {selected ? (
          <SetlistDetail
            setlist={selected}
            tracksById={tracksById}
            tracks={tracks}
            presets={presets}
            onRename={handleRename}
            onAddEntry={handleAddEntry}
            onRemoveEntry={handleRemoveEntry}
            onMoveEntry={handleMoveEntry}
            onSetTransition={handleSetTransition}
            onLoadToDeck={onLoadToDeck}
          />
        ) : (
          <p className="hint">Select or create a setlist to start.</p>
        )}
      </div>
    </section>
  );
}

function SetlistDetail({
  setlist,
  tracksById,
  tracks,
  presets,
  onRename,
  onAddEntry,
  onRemoveEntry,
  onMoveEntry,
  onSetTransition,
  onLoadToDeck,
}: {
  setlist: SetlistDto;
  tracksById: Map<string, TrackSummary>;
  tracks: TrackSummary[];
  presets: TemplatePreset[];
  onRename: (id: string, name: string) => void | Promise<void>;
  onAddEntry: (trackId: string) => void | Promise<void>;
  onRemoveEntry: (entryId: string) => void | Promise<void>;
  onMoveEntry: (entryId: string, newIndex: number) => void | Promise<void>;
  onSetTransition: (
    entryId: string,
    spec: TransitionSpec | null,
  ) => void | Promise<void>;
  onLoadToDeck: (deck: DeckId, path: string) => void;
}) {
  const [nameDraft, setNameDraft] = useState(setlist.name);
  useEffect(() => {
    setNameDraft(setlist.name);
  }, [setlist.id, setlist.name]);

  const [pickerQuery, setPickerQuery] = useState("");
  const filteredPickerTracks = useMemo(() => {
    const q = pickerQuery.trim().toLowerCase();
    if (!q) return tracks;
    return tracks.filter(
      (t) =>
        t.title.toLowerCase().includes(q) ||
        t.artist.toLowerCase().includes(q),
    );
  }, [tracks, pickerQuery]);

  return (
    <>
      <header className="setlist-detail-header">
        <input
          type="text"
          className="setlist-name-input"
          value={nameDraft}
          onChange={(e) => setNameDraft(e.target.value)}
          onBlur={() => {
            if (nameDraft.trim() && nameDraft !== setlist.name) {
              void onRename(setlist.id, nameDraft);
            } else if (!nameDraft.trim()) {
              setNameDraft(setlist.name);
            }
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") (e.target as HTMLInputElement).blur();
            if (e.key === "Escape") {
              setNameDraft(setlist.name);
              (e.target as HTMLInputElement).blur();
            }
          }}
        />
        <span className="setlist-detail-meta">
          {setlist.entries.length} track{setlist.entries.length === 1 ? "" : "s"}
        </span>
      </header>

      <div className="setlist-entries">
        {setlist.entries.length === 0 ? (
          <p className="hint">Empty setlist. Add tracks from the picker below.</p>
        ) : (
          <ol className="setlist-entry-list">
            {setlist.entries.map((entry, idx) => (
              <li key={entry.id} className="setlist-entry">
                <SetlistEntryRow
                  entry={entry}
                  index={idx}
                  total={setlist.entries.length}
                  track={tracksById.get(entry.track_id) ?? null}
                  presets={presets}
                  onMoveUp={() =>
                    idx > 0 && void onMoveEntry(entry.id, idx - 1)
                  }
                  onMoveDown={() =>
                    idx < setlist.entries.length - 1 &&
                    void onMoveEntry(entry.id, idx + 1)
                  }
                  onRemove={() => void onRemoveEntry(entry.id)}
                  onSetTransition={(spec) =>
                    void onSetTransition(entry.id, spec)
                  }
                  onLoadToDeck={onLoadToDeck}
                />
              </li>
            ))}
          </ol>
        )}
      </div>

      <div className="setlist-picker">
        <header>
          <strong>Add track</strong>
          <input
            type="text"
            placeholder="search title / artist"
            value={pickerQuery}
            onChange={(e) => setPickerQuery(e.target.value)}
            className="search-input"
          />
        </header>
        <ul className="setlist-picker-list">
          {filteredPickerTracks.slice(0, 50).map((t) => (
            <li key={t.id}>
              <span className="picker-title">{t.title || "(untitled)"}</span>
              <span className="picker-artist">{t.artist || "—"}</span>
              <span className="picker-bpm tabular">
                {t.bpm > 0 ? `${t.bpm.toFixed(1)} / ${t.key}` : "—"}
              </span>
              <button className="chip" onClick={() => void onAddEntry(t.id)}>
                +
              </button>
            </li>
          ))}
          {filteredPickerTracks.length === 0 && (
            <li className="hint">No tracks match.</li>
          )}
        </ul>
      </div>
    </>
  );
}

function SetlistEntryRow({
  entry,
  index,
  total,
  track,
  presets,
  onMoveUp,
  onMoveDown,
  onRemove,
  onSetTransition,
  onLoadToDeck,
}: {
  entry: SetlistEntryDto;
  index: number;
  total: number;
  track: TrackSummary | null;
  presets: TemplatePreset[];
  onMoveUp: () => void;
  onMoveDown: () => void;
  onRemove: () => void;
  onSetTransition: (spec: TransitionSpec | null) => void;
  onLoadToDeck: (deck: DeckId, path: string) => void;
}) {
  const isLast = index === total - 1;
  const tx = entry.transition_to_next;

  return (
    <div className="setlist-entry-card">
      <div className="setlist-entry-head">
        <span className="setlist-entry-index">{index + 1}</span>
        <div className="setlist-entry-track">
          {track ? (
            <>
              <span className="setlist-entry-title">
                {track.title || "(untitled)"}
              </span>
              <span className="setlist-entry-sub">
                {track.artist || "—"} ·{" "}
                {track.bpm > 0 ? `${track.bpm.toFixed(1)} BPM / ${track.key}` : "—"}
              </span>
            </>
          ) : (
            <em className="muted">missing track (deleted from library)</em>
          )}
        </div>
        <div className="setlist-entry-actions">
          <button
            className="chip"
            disabled={index === 0}
            onClick={onMoveUp}
            title="Move up"
          >
            ↑
          </button>
          <button
            className="chip"
            disabled={isLast}
            onClick={onMoveDown}
            title="Move down"
          >
            ↓
          </button>
          <button
            className="chip"
            disabled={!track}
            onClick={() => track && onLoadToDeck("A", track.path)}
            title="Load to Deck A"
          >
            →A
          </button>
          <button
            className="chip"
            disabled={!track}
            onClick={() => track && onLoadToDeck("B", track.path)}
            title="Load to Deck B"
          >
            →B
          </button>
          <button className="chip danger" onClick={onRemove} title="Remove">
            ×
          </button>
        </div>
      </div>

      {!isLast && (
        <div className="setlist-transition">
          <span className="setlist-transition-label">→ Transition</span>
          <select
            value={tx?.template_id ?? ""}
            onChange={(e) => {
              const presetId = e.target.value;
              if (!presetId) {
                onSetTransition(null);
                return;
              }
              onSetTransition({
                template_id: presetId,
                tempo_mode: tx?.tempo_mode ?? "linear_blend",
                entry_cue: tx?.entry_cue ?? null,
                exit_cue: tx?.exit_cue ?? null,
              });
            }}
          >
            <option value="">(no transition)</option>
            {presets.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name} ({p.duration_beats}b)
              </option>
            ))}
          </select>
          {tx && (
            <select
              value={tx.tempo_mode}
              onChange={(e) =>
                onSetTransition({
                  ...tx,
                  tempo_mode: e.target.value as TempoMode,
                })
              }
            >
              {(Object.keys(TEMPO_MODE_LABELS) as TempoMode[]).map((m) => (
                <option key={m} value={m}>
                  {TEMPO_MODE_LABELS[m]}
                </option>
              ))}
            </select>
          )}
        </div>
      )}
    </div>
  );
}
