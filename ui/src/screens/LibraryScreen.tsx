import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useMemo, useState } from "react";

import { ipc, type ExportPreview } from "@/lib/ipc";
import type { DeckId } from "@/types/mixer";
import type { TrackSummary } from "@/types/track";

interface LibraryScreenProps {
  tracks: TrackSummary[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  onLoadToDeck: (deck: DeckId, path: string) => void;
}

export function LibraryScreen({
  tracks,
  loading,
  error,
  refresh,
  onLoadToDeck,
}: LibraryScreenProps) {
  const [search, setSearch] = useState("");
  const [importing, setImporting] = useState(false);

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return tracks;
    return tracks.filter(
      (t) =>
        t.title.toLowerCase().includes(q) ||
        t.artist.toLowerCase().includes(q) ||
        t.album.toLowerCase().includes(q),
    );
  }, [tracks, search]);

  const handleImport = useCallback(async () => {
    const selection = await open({
      multiple: true,
      filters: [
        {
          name: "Audio",
          extensions: ["mp3", "wav", "flac", "aac", "ogg", "m4a"],
        },
      ],
    });
    if (!selection) return;
    const list = Array.isArray(selection) ? selection : [selection];
    setImporting(true);
    try {
      for (const p of list) {
        try {
          await ipc.importTrack(p);
        } catch (e) {
          console.error("import failed:", p, e);
        }
      }
      await refresh();
    } finally {
      setImporting(false);
    }
  }, [refresh]);

  const handleDelete = useCallback(
    async (id: string) => {
      await ipc.deleteTrack(id);
      await refresh();
    },
    [refresh],
  );

  const [exportInfo, setExportInfo] = useState<
    | { state: "idle" }
    | { state: "previewing" }
    | { state: "preview"; preview: ExportPreview }
    | { state: "error"; error: string }
  >({ state: "idle" });

  const [demoCueResult, setDemoCueResult] = useState<string | null>(null);
  const handleInjectDemoCues = useCallback(async () => {
    setDemoCueResult("Injecting…");
    try {
      const n = await ipc.injectDemoCues();
      setDemoCueResult(
        n === 0
          ? "All tracks already have a DROP/Entry cue."
          : `Inserted ${n} demo DROP cue${n === 1 ? "" : "s"} (Entry, 32b).`,
      );
      await refresh();
    } catch (e) {
      setDemoCueResult(`Failed: ${e}`);
    }
  }, [refresh]);

  const handleExportPreview = useCallback(async () => {
    const dest = await open({ directory: true, multiple: false });
    if (!dest || Array.isArray(dest)) return;
    setExportInfo({ state: "previewing" });
    try {
      const preview = await ipc.exportPreview(dest);
      setExportInfo({ state: "preview", preview });
    } catch (e) {
      setExportInfo({ state: "error", error: String(e) });
    }
  }, []);

  const [analyzingId, setAnalyzingId] = useState<string | null>(null);
  const handleAnalyze = useCallback(
    async (id: string) => {
      setAnalyzingId(id);
      try {
        await ipc.analyzeTrack(id);
        await refresh();
      } catch (e) {
        console.error("analyze failed:", id, e);
      } finally {
        setAnalyzingId(null);
      }
    },
    [refresh],
  );

  return (
    <section className="library">
      <div className="library-toolbar">
        <input
          type="text"
          placeholder="search title / artist / album"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="search-input"
        />
        <button className="btn" onClick={handleImport} disabled={importing}>
          {importing ? "Importing…" : "Import…"}
        </button>
        <button className="btn" onClick={refresh} disabled={loading}>
          Refresh
        </button>
        <button
          className="btn"
          onClick={handleInjectDemoCues}
          disabled={tracks.length === 0}
          title="Insert one DROP @ 32-beat Entry cue per track (dev / test only)"
        >
          + Demo Cues
        </button>
        <button
          className="btn"
          onClick={handleExportPreview}
          disabled={exportInfo.state === "previewing" || tracks.length === 0}
          title="Preview a rekordbox-compatible USB export (Phase 1: dry-run only)"
        >
          {exportInfo.state === "previewing" ? "Previewing…" : "Export to USB…"}
        </button>
        <span className="track-count">
          {filtered.length}
          {filtered.length !== tracks.length && <> / {tracks.length}</>} tracks
        </span>
      </div>

      {error && <p className="hint" style={{ color: "var(--c-danger)" }}>{error}</p>}
      {demoCueResult && (
        <p className="hint" style={{ color: "var(--c-ink-9)" }}>
          {demoCueResult}
        </p>
      )}

      {exportInfo.state === "preview" && (
        <div className="export-preview">
          <header>
            <strong>Export preview</strong>
            <button className="chip" onClick={() => setExportInfo({ state: "idle" })}>
              ×
            </button>
          </header>
          <dl>
            <div>
              <dt>Destination</dt>
              <dd className="mono">{exportInfo.preview.root}</dd>
            </div>
            <div>
              <dt>Tracks</dt>
              <dd>{exportInfo.preview.track_count}</dd>
            </div>
            <div>
              <dt>Audio bytes</dt>
              <dd>{formatBytes(exportInfo.preview.estimated_audio_bytes)}</dd>
            </div>
            <div>
              <dt>With beatgrid</dt>
              <dd>{exportInfo.preview.tracks_with_beatgrid}</dd>
            </div>
            <div>
              <dt>With waveform</dt>
              <dd>{exportInfo.preview.tracks_with_waveform}</dd>
            </div>
            <div>
              <dt>Hot cues</dt>
              <dd>{exportInfo.preview.total_hot_cues}</dd>
            </div>
          </dl>
          <p className="hint">
            Phase 1: プラン構築までの dry-run。実際の <code>export.pdb</code> /{" "}
            <code>.DAT</code> / <code>.EXT</code> 書き出しは Phase 2 以降で実装します。
          </p>
        </div>
      )}
      {exportInfo.state === "error" && (
        <p className="hint" style={{ color: "var(--c-danger)" }}>
          Export preview failed: {exportInfo.error}
        </p>
      )}

      <div className="tracklist">
        <table>
          <thead>
            <tr>
              <th style={{ width: "26%" }}>Title</th>
              <th style={{ width: "18%" }}>Artist</th>
              <th style={{ width: "14%" }}>Album</th>
              <th style={{ width: "8%" }}>BPM</th>
              <th style={{ width: "6%" }}>Key</th>
              <th style={{ width: "8%" }}>Time</th>
              <th style={{ width: "10%" }}>Status</th>
              <th style={{ width: "10%" }}></th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((t) => (
              <tr key={t.id}>
                <td className="cell-primary">
                  {t.title || <em className="muted">(untitled)</em>}
                </td>
                <td>{t.artist || <em className="muted">—</em>}</td>
                <td>{t.album || <em className="muted">—</em>}</td>
                <td className="tabular">{t.bpm > 0 ? t.bpm.toFixed(1) : "—"}</td>
                <td className="tabular">{t.bpm > 0 ? t.key : "—"}</td>
                <td className="tabular">{formatSec(t.duration_sec)}</td>
                <td>
                  <StatusBadge track={t} />
                </td>
                <td className="actions">
                  <button
                    className="chip"
                    onClick={() => handleAnalyze(t.id)}
                    disabled={analyzingId === t.id}
                    title="Re-analyze (regenerate waveform)"
                  >
                    {analyzingId === t.id ? "…" : "↻"}
                  </button>
                  <button
                    className="chip"
                    onClick={() => onLoadToDeck("A", t.path)}
                    title="Load to Deck A"
                  >
                    →A
                  </button>
                  <button
                    className="chip"
                    onClick={() => onLoadToDeck("B", t.path)}
                    title="Load to Deck B"
                  >
                    →B
                  </button>
                  <button
                    className="chip danger"
                    onClick={() => handleDelete(t.id)}
                    title="Remove from library"
                  >
                    ×
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {filtered.length === 0 && !loading && (
          <p className="hint">
            {tracks.length === 0
              ? "No tracks yet. Click Import to add audio files."
              : `No matches for "${search}".`}
          </p>
        )}
      </div>
    </section>
  );
}

function StatusBadge({
  track,
}: {
  track: { analyzed: boolean; beatgrid_verified: boolean };
}) {
  if (track.analyzed && track.beatgrid_verified) {
    return <span className="badge good">ready</span>;
  }
  if (track.analyzed) {
    return <span className="badge warn">unverified</span>;
  }
  return <span className="badge muted">raw</span>;
}

function formatSec(sec: number): string {
  if (!Number.isFinite(sec) || sec < 0) return "0:00";
  const m = Math.floor(sec / 60);
  const s = Math.floor(sec % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

function formatBytes(n: number): string {
  if (!Number.isFinite(n) || n <= 0) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v >= 100 ? 0 : v >= 10 ? 1 : 2)} ${units[i]}`;
}
