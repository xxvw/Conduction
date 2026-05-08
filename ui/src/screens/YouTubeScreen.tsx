import { useEffect, useState } from "react";

import { ipc, type AudioFormat, type VideoSearchResult } from "@/lib/ipc";

const FORMATS: AudioFormat[] = ["m4a", "mp3", "opus", "wav", "flac"];

interface YouTubeScreenProps {
  onImported: () => void;
}

type AvailabilityState = "checking" | "ready" | "missing";

export function YouTubeScreen({ onImported }: YouTubeScreenProps) {
  const [availability, setAvailability] = useState<AvailabilityState>("checking");
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<VideoSearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [format, setFormat] = useState<AudioFormat>("m4a");
  const [downloading, setDownloading] = useState<Record<string, boolean>>({});
  const [downloadMessage, setDownloadMessage] = useState<string | null>(null);

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
    setDownloading((d) => ({ ...d, [item.id]: true }));
    setDownloadMessage(`Downloading "${item.title}"…`);
    try {
      const summary = await ipc.ytDownload(item.url, format);
      setDownloadMessage(`Imported: ${summary.title || summary.path}`);
      onImported();
    } catch (e) {
      setDownloadMessage(`Download failed: ${e}`);
    } finally {
      setDownloading((d) => ({ ...d, [item.id]: false }));
    }
  }

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
      {downloadMessage && <p className="youtube-message">{downloadMessage}</p>}

      <div className="youtube-results">
        {results.map((r) => (
          <article key={r.id} className="youtube-card">
            {r.thumbnail ? (
              <img className="youtube-thumb" src={r.thumbnail} alt="" loading="lazy" />
            ) : (
              <div className="youtube-thumb youtube-thumb-placeholder" />
            )}
            <div className="youtube-meta">
              <a className="youtube-title" href={r.url} target="_blank" rel="noreferrer">
                {r.title}
              </a>
              <div className="youtube-sub">
                <span>{r.channel || "—"}</span>
                {r.duration_sec != null && <span>{formatDuration(r.duration_sec)}</span>}
                {r.view_count != null && <span>{formatViews(r.view_count)} views</span>}
              </div>
            </div>
            <button
              className="btn"
              disabled={!!downloading[r.id]}
              onClick={() => void handleDownload(r)}
            >
              {downloading[r.id] ? "Downloading…" : `Download ${format}`}
            </button>
          </article>
        ))}
        {!searching && results.length === 0 && (
          <p className="hint">検索ワードを入力してください。例: "lo-fi hip hop", "deep house mix"</p>
        )}
      </div>
    </section>
  );
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
