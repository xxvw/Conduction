import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent } from "react";

import { ipc, type SetlistDto } from "@/lib/ipc";
import type { DeckId } from "@/types/mixer";
import type { PerformanceBrowser } from "@/types/performance";
import type { TrackSummary } from "@/types/track";

interface ControllerLibraryProps {
  tracks: TrackSummary[];
  onLoadToDeck: (deck: DeckId, path: string) => Promise<void> | void;
  onError: (message: string) => void;
  browser?: PerformanceBrowser;
  onBrowserChange?: (browser: PerformanceBrowser) => Promise<void>;
}

interface BrowserRow {
  id: string;
  track: TrackSummary;
}

function filterRows(tracks: TrackSummary[], setlists: SetlistDto[], setlistId: string, query: string): BrowserRow[] {
  const setlist = setlists.find((item) => item.id === setlistId);
  const byId = new Map(tracks.map((track) => [track.id, track]));
  const source = setlist
    ? setlist.entries.flatMap((entry) => {
        const track = byId.get(entry.track_id);
        return track ? [{ id: entry.id, track }] : [];
      })
    : tracks.map((track) => ({ id: track.id, track }));
  const words = query.trim().toLocaleLowerCase().split(/\s+/u).filter(Boolean);
  return source.filter(({ track }) => {
    const text = `${track.title} ${track.artist} ${track.album} ${track.key}`.toLocaleLowerCase();
    return words.every((word) => text.includes(word));
  });
}

function duration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "—";
  const whole = Math.floor(seconds);
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, "0")}`;
}

export function ControllerLibrary({
  tracks,
  onLoadToDeck,
  onError,
  browser,
  onBrowserChange,
}: ControllerLibraryProps) {
  const [query, setQuery] = useState(browser?.query ?? "");
  const [setlists, setSetlists] = useState<SetlistDto[]>([]);
  const [setlistId, setSetlistId] = useState(browser?.playlist_id ?? "");
  const [refreshing, setRefreshing] = useState(false);
  const [selectedRowId, setSelectedRowId] = useState<string | null>(null);
  const [loadDeck, setLoadDeck] = useState<DeckId>("A");
  const [busy, setBusy] = useState<Partial<Record<DeckId, string>>>({});
  const [loadStatus, setLoadStatus] = useState("");
  const busyRef = useRef<Partial<Record<DeckId, string>>>({});
  const errorRef = useRef(onError);
  errorRef.current = onError;
  const alive = useRef(false);
  const refreshRequest = useRef(0);
  const rowsRef = useRef(new Map<string, HTMLTableRowElement>());
  const tableWrapRef = useRef<HTMLDivElement>(null);
  const searchFocused = useRef(false);
  const browserWriter = useRef(onBrowserChange);
  browserWriter.current = onBrowserChange;
  const pendingBrowser = useRef<PerformanceBrowser | null>(null);
  const publishingBrowser = useRef(false);

  // Serialize writes and retain only the latest queued state, so fast typing
  // cannot let an older response become the final MIDI browser state.
  const publishBrowser = useCallback((next: PerformanceBrowser) => {
    if (!browserWriter.current) return;
    pendingBrowser.current = next;
    if (publishingBrowser.current) return;
    publishingBrowser.current = true;
    void (async () => {
      try {
        while (pendingBrowser.current && alive.current) {
          const update = pendingBrowser.current;
          pendingBrowser.current = null;
          try {
            await browserWriter.current?.(update);
          } catch (error) {
            if (alive.current) errorRef.current(`ブラウザーの同期に失敗しました: ${String(error)}`);
          }
        }
      } finally {
        publishingBrowser.current = false;
      }
    })();
  }, []);

  const refreshSetlists = useCallback(async () => {
    const request = ++refreshRequest.current;
    setRefreshing(true);
    try {
      const result = await ipc.listSetlists();
      if (!alive.current || request !== refreshRequest.current) return;
      setSetlists(result);
      setSetlistId((id) => result.some((setlist) => setlist.id === id) ? id : "");
    } catch (error) {
      if (alive.current && request === refreshRequest.current) {
        errorRef.current(`セットリストを取得できませんでした: ${String(error)}`);
      }
    } finally {
      if (alive.current && request === refreshRequest.current) setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    alive.current = true;
    void refreshSetlists();
    return () => {
      alive.current = false;
      refreshRequest.current += 1;
    };
  }, [refreshSetlists]);

  const rows = useMemo(() => filterRows(tracks, setlists, setlistId, query), [tracks, setlists, setlistId, query]);
  const setlistsRef = useRef(setlists);
  setlistsRef.current = setlists;
  // Only actual backend changes replace local state. Ignore acknowledgements
  // during writes and protect a search that the performer is actively typing.
  useEffect(() => {
    if (!browser || publishingBrowser.current || searchFocused.current) return;
    setQuery(browser.query);
    setSetlistId(browser.playlist_id ?? "");
    const entry = setlistsRef.current.find((setlist) => setlist.id === browser.playlist_id)?.entries.find((item) => item.track_id === browser.selected_track_id);
    setSelectedRowId(entry?.id ?? browser.selected_track_id);
  }, [browser?.query, browser?.selected_track_id, browser?.playlist_id]);

  const selection = rows.find((row) => row.id === selectedRowId || row.track.id === selectedRowId) ?? rows[0];

  const selectRow = (row: BrowserRow) => {
    setSelectedRowId(row.id);
    publishBrowser({ query, selected_track_id: row.track.id, playlist_id: setlistId || null });
  };

  const changeFilter = (nextQuery: string, nextSetlistId: string) => {
    const nextRows = filterRows(tracks, setlists, nextSetlistId, nextQuery);
    const next = nextRows.find((row) => row.id === selectedRowId) ?? nextRows[0];
    setQuery(nextQuery);
    setSetlistId(nextSetlistId);
    setSelectedRowId(next?.id ?? null);
    publishBrowser({ query: nextQuery, selected_track_id: next?.track.id ?? null, playlist_id: nextSetlistId || null });
  };

  useEffect(() => {
    const row = selection ? rowsRef.current.get(selection.id) : undefined;
    const container = tableWrapRef.current;
    if (!row || !container) return;
    const rowBounds = row.getBoundingClientRect();
    const bounds = container.getBoundingClientRect();
    const headerHeight = container.querySelector("thead")?.getBoundingClientRect().height ?? 0;
    if (rowBounds.top < bounds.top + headerHeight) container.scrollTop += rowBounds.top - bounds.top - headerHeight;
    else if (rowBounds.bottom > bounds.bottom) container.scrollTop += rowBounds.bottom - bounds.bottom;
  }, [selection?.id]);

  const load = async (deck: DeckId, track: TrackSummary) => {
    if (busyRef.current[deck]) return;
    busyRef.current = { ...busyRef.current, [deck]: track.id };
    setBusy({ ...busyRef.current });
    setLoadStatus(`Deck ${deck} に ${track.title || "無題"} を読み込み中…`);
    try {
      await onLoadToDeck(deck, track.path);
      if (alive.current) setLoadStatus(`Deck ${deck}: ${track.title || "無題"} を読み込みました`);
    } catch (error) {
      if (alive.current) {
        setLoadStatus(`Deck ${deck}: 読み込みに失敗しました`);
        errorRef.current(`Deck ${deck} の読み込みに失敗しました: ${String(error)}`);
      }
    } finally {
      delete busyRef.current[deck];
      if (alive.current) setBusy({ ...busyRef.current });
    }
  };

  const handleKeys = (event: KeyboardEvent<HTMLDivElement>) => {
    const target = event.target as HTMLElement;
    if (target.closest("button, input, select, textarea")) return;
    if (event.altKey || event.ctrlKey || event.metaKey) return;
    const index = selection ? rows.indexOf(selection) : -1;
    let next: BrowserRow | undefined;
    if (event.key === "ArrowDown") next = rows[Math.min(rows.length - 1, index + 1)];
    else if (event.key === "ArrowUp") next = rows[Math.max(0, index - 1)];
    else if (event.key === "Home") next = rows[0];
    else if (event.key === "End") next = rows[rows.length - 1];
    else if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
      event.preventDefault();
      setLoadDeck(event.key === "ArrowLeft" ? "A" : "B");
    } else if (event.key === "Enter" && selection) {
      event.preventDefault();
      if (!event.repeat) void load(loadDeck, selection.track);
    }
    if (next) {
      event.preventDefault();
      selectRow(next);
      rowsRef.current.get(next.id)?.focus({ preventScroll: true });
    }
  };

  return (
    <section className="ctl-library" aria-label="Controller library" onKeyDown={(event) => event.stopPropagation()}>
      <header className="ctl-library-header">
        <div className="ctl-library-title">
          <strong>LIBRARY</strong>
          <span className="ctl-muted">{rows.length} tracks</span>
        </div>
        <label className="ctl-library-source">
          <span className="ctl-sr-only">ライブラリまたはセットリスト</span>
          <select value={setlistId} onChange={(event) => changeFilter(query, event.target.value)}>
            <option value="">すべての曲</option>
            {setlists.map((setlist) => (
              <option key={setlist.id} value={setlist.id}>{setlist.name}</option>
            ))}
          </select>
        </label>
        <button type="button" className="ctl-button ctl-library-refresh" onClick={() => void refreshSetlists()} disabled={refreshing} aria-label="セットリストを更新">
          {refreshing ? "更新中…" : "更新"}
        </button>
        <label className="ctl-library-search">
          <span className="ctl-sr-only">曲名・アーティスト・アルバム・キーを検索</span>
          <input type="search" value={query} onFocus={() => { searchFocused.current = true; }} onBlur={() => { searchFocused.current = false; }} onChange={(event) => changeFilter(event.target.value, setlistId)} placeholder="曲名 / アーティスト / アルバム / KEY" />
        </label>
        <div className="ctl-segmented" role="group" aria-label="Enter キーで読み込むデッキ">
          {(["A", "B"] as const).map((deck) => (
            <button key={deck} type="button" className={`ctl-button ctl-deck-${deck.toLowerCase()}${loadDeck === deck ? " is-active" : ""}`} aria-pressed={loadDeck === deck} onClick={() => setLoadDeck(deck)}>LOAD {deck}</button>
          ))}
        </div>
      </header>
      <div ref={tableWrapRef} className="ctl-library-table-wrap" tabIndex={0} onKeyDown={handleKeys} role="region" aria-label="曲一覧。上下矢印で選曲、左右矢印でデッキ選択、Enter で読み込み">
        <table className="ctl-library-table">
          <thead><tr><th scope="col">TITLE</th><th scope="col">ARTIST</th><th scope="col">BPM</th><th scope="col">KEY</th><th scope="col">TIME</th><th scope="col">LOAD</th></tr></thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.id} ref={(node) => { if (node) rowsRef.current.set(row.id, node); else rowsRef.current.delete(row.id); }} tabIndex={row.id === selection?.id ? 0 : -1} className={row.id === selection?.id ? "is-selected" : undefined} onClick={() => selectRow(row)} onFocus={() => selectRow(row)}>
                <td className="ctl-library-track" title={row.track.path}>{row.track.title || "無題"}{!row.track.analyzed && <span className="ctl-track-badge" title="Library で解析すると BPM・キー・波形が利用できます">未解析</span>}</td>
                <td className="ctl-library-artist">{row.track.artist || "—"}</td>
                <td className="ctl-number">{row.track.bpm > 0 && Number.isFinite(row.track.bpm) ? row.track.bpm.toFixed(1) : "—"}</td>
                <td className="ctl-number">{row.track.key || "—"}</td>
                <td className="ctl-number">{duration(row.track.duration_sec)}</td>
                <td><div className="ctl-library-loads">{(["A", "B"] as const).map((deck) => (
                  <button key={deck} type="button" className={`ctl-button ctl-deck-${deck.toLowerCase()}`} disabled={Boolean(busy[deck])} aria-label={`${row.track.title || "無題"} を Deck ${deck} に読み込み`} aria-busy={busy[deck] === row.track.id} onClick={() => void load(deck, row.track)}>{busy[deck] === row.track.id ? "…" : deck}</button>
                ))}</div></td>
              </tr>
            ))}
          </tbody>
        </table>
        {rows.length === 0 && <p className="ctl-empty">{tracks.length === 0 ? "Library 画面から曲をインポートしてください。ここから Deck A / B に読み込めます。" : query.trim() ? "検索条件に一致する曲がありません。" : "このセットリストには利用可能な曲がありません。"}</p>}
      </div>
      <footer className="ctl-library-footer">
        <span className="ctl-muted">↑↓ 選曲 · ←→ デッキ選択 · Enter → Deck {loadDeck}</span>
        <span role="status" aria-live="polite">{loadStatus}</span>
      </footer>
    </section>
  );
}
