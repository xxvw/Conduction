import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useMemo, useRef, useState } from "react";

import {
  ipc,
  type AudioFormat,
  type VideoSearchResult,
  type YtProgressEvent,
} from "@/lib/ipc";

const FORMATS: AudioFormat[] = ["m4a", "mp3", "opus", "wav", "flac"];

interface YouTubeScreenProps {
  onImported: () => void;
}

type AvailabilityState = "checking" | "ready" | "missing";

interface DownloadState {
  /** 0..100 */
  percent: number;
  stage: "download" | "postprocess" | "other";
  message: string;
  done: boolean;
  error: string | null;
}

export function YouTubeScreen({ onImported }: YouTubeScreenProps) {
  const [availability, setAvailability] = useState<AvailabilityState>("checking");
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<VideoSearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [format, setFormat] = useState<AudioFormat>("m4a");
  const [downloads, setDownloads] = useState<Record<string, DownloadState>>({});
  const requestIdToVideoId = useRef<Map<string, string>>(new Map());

  // yt:progress / yt:done を listen して、対応する video の進捗を更新。
  useEffect(() => {
    let unlistenProgress: UnlistenFn | undefined;
    let unlistenDone: UnlistenFn | undefined;
    void listen<YtProgressEvent>("yt:progress", (e) => {
      const videoId = requestIdToVideoId.current.get(e.payload.request_id);
      if (!videoId) return;
      setDownloads((prev) => {
        const cur = prev[videoId] ?? makeInitialState();
        return {
          ...prev,
          [videoId]: {
            ...cur,
            percent:
              e.payload.percent != null && Number.isFinite(e.payload.percent)
                ? e.payload.percent
                : cur.percent,
            stage: e.payload.stage,
            message: e.payload.raw,
          },
        };
      });
    }).then((fn) => {
      unlistenProgress = fn;
    });
    void listen<{ request_id: string; ok: boolean }>("yt:done", (e) => {
      const videoId = requestIdToVideoId.current.get(e.payload.request_id);
      if (!videoId) return;
      setDownloads((prev) => {
        const cur = prev[videoId] ?? makeInitialState();
        return {
          ...prev,
          [videoId]: { ...cur, done: true, percent: e.payload.ok ? 100 : cur.percent },
        };
      });
    }).then((fn) => {
      unlistenDone = fn;
    });
    return () => {
      unlistenProgress?.();
      unlistenDone?.();
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    ipc
      .ytDlpAvailable()
      .then((ok) => {
        if (cancelled) return;
        setAvailability(ok ? "ready" : "missing");
      })
      .catch(() => {
        if (!cancelled) setAvailability("missing");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function handleSearch(e: React.FormEvent) {
    e.preventDefault();
    if (!query.trim() || availability !== "ready") return;
    setSearching(true);
    setSearchError(null);
    try {
      const r = await ipc.ytSearch(query.trim(), 15);
      setResults(r);
    } catch (e) {
      setSearchError(String(e));
      setResults([]);
    } finally {
      setSearching(false);
    }
  }

  async function handleDownload(item: VideoSearchResult) {
    const requestId =
      (typeof crypto !== "undefined" && "randomUUID" in crypto
        ? crypto.randomUUID()
        : `${Date.now()}-${Math.random().toString(36).slice(2)}`);
    requestIdToVideoId.current.set(requestId, item.id);
    setDownloads((prev) => ({ ...prev, [item.id]: makeInitialState() }));
    try {
      await ipc.ytDownload(item.url, format, requestId);
      setDownloads((prev) => ({
        ...prev,
        [item.id]: { ...(prev[item.id] ?? makeInitialState()), done: true, percent: 100 },
      }));
      onImported();
    } catch (e) {
      setDownloads((prev) => ({
        ...prev,
        [item.id]: {
          ...(prev[item.id] ?? makeInitialState()),
          done: true,
          error: String(e),
        },
      }));
    } finally {
      requestIdToVideoId.current.delete(requestId);
    }
  }

  // サムネイルは検索結果が来たあと 1 件ずつ順番に解放する。
  const revealedThumbs = useStaggeredReveal(results.length, results.map((r) => r.id).join("|"));

  if (availability === "checking") {
    return (
      <section className="youtube-screen">
        <p className="hint">Checking yt-dlp availability…</p>
      </section>
    );
  }
  if (availability === "missing") {
    return (
      <section className="youtube-screen">
        <header className="youtube-header">
          <h2>YouTube</h2>
        </header>
        <div className="youtube-missing">
          <p>
            <code>yt-dlp</code> がシステムの PATH に見つかりません。
          </p>
          <p className="hint">
            Homebrew なら <code>brew install yt-dlp</code>、pip なら{" "}
            <code>pip install yt-dlp</code> でインストールしてからアプリを再起動してください。
          </p>
        </div>
      </section>
    );
  }

  return (
    <section className="youtube-screen">
      <header className="youtube-header">
        <h2>YouTube</h2>
        <div className="youtube-format">
          <label htmlFor="yt-format">Format</label>
          <select
            id="yt-format"
            value={format}
            onChange={(e) => setFormat(e.target.value as AudioFormat)}
          >
            {FORMATS.map((f) => (
              <option key={f} value={f}>
                {f}
              </option>
            ))}
          </select>
        </div>
      </header>

      <form className="youtube-search" onSubmit={handleSearch}>
        <input
          type="search"
          placeholder="Search YouTube…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          autoFocus
        />
        <button type="submit" className="btn" disabled={searching || !query.trim()}>
          {searching ? "Searching…" : "Search"}
        </button>
      </form>

      {searchError && <p className="youtube-error">{searchError}</p>}

      <div className="youtube-results">
        {results.map((r, idx) => (
          <ResultCard
            key={r.id}
            item={r}
            showThumb={revealedThumbs > idx}
            format={format}
            download={downloads[r.id]}
            onDownload={() => void handleDownload(r)}
          />
        ))}
        {!searching && results.length === 0 && (
          <p className="hint">検索ワードを入力してください。例: "lo-fi hip hop", "deep house mix"</p>
        )}
      </div>
    </section>
  );
}

function ResultCard({
  item,
  showThumb,
  format,
  download,
  onDownload,
}: {
  item: VideoSearchResult;
  showThumb: boolean;
  format: AudioFormat;
  download: DownloadState | undefined;
  onDownload: () => void;
}) {
  const inProgress = download != null && !download.done;
  const isDone = download?.done && !download.error;
  const isError = !!download?.error;

  const buttonLabel = useMemo(() => {
    if (isError) return "Retry";
    if (isDone) return "Imported ✓";
    if (inProgress) {
      const p = Math.max(0, Math.min(100, download?.percent ?? 0));
      const stageLabel =
        download?.stage === "postprocess" ? "Post" : download?.stage === "download" ? "DL" : "…";
      return `${stageLabel} ${p.toFixed(0)}%`;
    }
    return `Download ${format}`;
  }, [isError, isDone, inProgress, download, format]);

  return (
    <article className="youtube-card">
      {showThumb && item.thumbnail ? (
        <img className="youtube-thumb" src={item.thumbnail} alt="" />
      ) : (
        <div className="youtube-thumb youtube-thumb-placeholder" />
      )}
      <div className="youtube-meta">
        <a className="youtube-title" href={item.url} target="_blank" rel="noreferrer">
          {item.title}
        </a>
        <div className="youtube-sub">
          <span>{item.channel || "—"}</span>
          {item.duration_sec != null && <span>{formatDuration(item.duration_sec)}</span>}
          {item.view_count != null && <span>{formatViews(item.view_count)} views</span>}
        </div>
        {download && (inProgress || isError) && (
          <div className="youtube-progress">
            <div className="youtube-progress-bar">
              <div
                className="youtube-progress-fill"
                data-stage={download.stage}
                style={{ width: `${Math.max(0, Math.min(100, download.percent))}%` }}
              />
            </div>
            <span className="youtube-progress-msg">
              {isError ? download.error : (download.message || "starting…")}
            </span>
          </div>
        )}
      </div>
      <button
        className="btn"
        disabled={inProgress || isDone}
        onClick={onDownload}
      >
        {buttonLabel}
      </button>
    </article>
  );
}

function useStaggeredReveal(total: number, key: string): number {
  const [count, setCount] = useState(0);
  useEffect(() => {
    setCount(0);
    if (total === 0) return;
    let cancelled = false;
    let i = 0;
    const tick = () => {
      if (cancelled) return;
      i += 1;
      setCount(i);
      if (i < total) {
        // 約 60fps で 1 件ずつ出す。サムネのデコードは Image 任せでも、
        // DOM への投入を間引くだけで初期描画が一気に詰まらない。
        setTimeout(tick, 50);
      }
    };
    setTimeout(tick, 16);
    return () => {
      cancelled = true;
    };
    // key は results の id 連結なので、内容が変わった時だけ再開。
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, total]);
  return count;
}

function makeInitialState(): DownloadState {
  return { percent: 0, stage: "other", message: "", done: false, error: null };
}

function formatDuration(sec: number): string {
  const s = Math.round(sec);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = s % 60;
  if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(r).padStart(2, "0")}`;
  return `${m}:${String(r).padStart(2, "0")}`;
}

function formatViews(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}
