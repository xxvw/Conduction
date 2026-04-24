import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useMemo, useState } from "react";

import { useTracks } from "@/hooks/useTracks";
import { ipc } from "@/lib/ipc";
import type { DeckId } from "@/types/mixer";

interface LibraryScreenProps {
  onLoadToDeck: (deck: DeckId, path: string) => void;
}

export function LibraryScreen({ onLoadToDeck }: LibraryScreenProps) {
  const { tracks, loading, error, refresh } = useTracks();
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
        <span className="track-count">
          {filtered.length}
          {filtered.length !== tracks.length && <> / {tracks.length}</>} tracks
        </span>
      </div>

      {error && <p className="hint" style={{ color: "var(--c-danger)" }}>{error}</p>}

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
