import type { SetlistDto, SetlistEntryDto, TemplatePreset } from "@/lib/ipc";
import type { TrackSummary } from "@/types/track";

interface SetlistOverviewProps {
  setlist: SetlistDto;
  tracksById: Map<string, TrackSummary>;
  presets: TemplatePreset[];
}

interface ResolvedTrack {
  title: string;
  bpm: number;
  key: string;
  energy: number;
  duration_sec: number;
}

interface ResolvedPair {
  entry: SetlistEntryDto;
  track: ResolvedTrack;
}

export function SetlistOverview({
  setlist,
  tracksById,
  presets,
}: SetlistOverviewProps) {
  const pairs: ResolvedPair[] = setlist.entries
    .map((entry) => {
      const t = tracksById.get(entry.track_id);
      if (!t) return null;
      return {
        entry,
        track: {
          title: t.title,
          bpm: t.bpm,
          key: t.key,
          energy: t.energy,
          duration_sec: t.duration_sec,
        },
      };
    })
    .filter((p): p is ResolvedPair => p != null);
  const resolved = pairs.map((p) => p.track);

  // 各 transition の duration を「前後 BPM の平均」で秒換算して合計から引く。
  // Tempo Mode によらず単純平均 (LinearBlend を概算とみなす)。
  const presetsById = new Map(presets.map((p) => [p.id, p] as const));
  const trackTotal = resolved.reduce((s, t) => s + t.duration_sec, 0);
  let overlapSec = 0;
  for (let i = 0; i < pairs.length - 1; i++) {
    const tx = pairs[i]!.entry.transition_to_next;
    if (!tx) continue;
    const preset = presetsById.get(tx.template_id);
    if (!preset) continue;
    const a = pairs[i]!.track.bpm;
    const b = pairs[i + 1]!.track.bpm;
    const bpm = a > 0 && b > 0 ? (a + b) / 2 : a > 0 ? a : b;
    if (bpm <= 0) continue;
    overlapSec += (preset.duration_beats * 60) / bpm;
  }
  const totalSec = Math.max(0, trackTotal - overlapSec);
  const mins = Math.floor(totalSec / 60);
  const secs = Math.floor(totalSec % 60);

  const knownBpm = resolved.filter((t) => t.bpm > 0).map((t) => t.bpm);
  const bpmRange =
    knownBpm.length > 0
      ? `${Math.min(...knownBpm).toFixed(0)}–${Math.max(...knownBpm).toFixed(0)}`
      : "—";
  const uniqKeys = new Set(resolved.filter((t) => t.key).map((t) => t.key));
  const transitionCount = Math.max(0, resolved.length - 1);

  return (
    <aside className="setlist-overview">
      <section className="overview-section">
        <div className="overview-overline">セット概要 / OVERVIEW</div>
        <div className="overview-total">
          <span className="overview-total-time tabular">
            {mins}:{secs.toString().padStart(2, "0")}
          </span>
          <span className="overview-total-sub">
            total · {resolved.length} track{resolved.length === 1 ? "" : "s"}
          </span>
        </div>
        <div className="overview-stats">
          <Stat label="BPM" labelEn="TEMPO" value={bpmRange} />
          <Stat label="Key" labelEn="CAMELOT" value={String(uniqKeys.size)} />
          <Stat
            label="遷移"
            labelEn="TRANSITIONS"
            value={String(transitionCount)}
          />
        </div>
      </section>

      <section className="overview-section">
        <div className="overview-overline">BPM カーブ / BPM CURVE</div>
        <BpmCurve tracks={resolved} />
      </section>

      <section className="overview-section">
        <div className="overview-overline">Key 遷移 / KEY PATH</div>
        <CamelotWheel tracks={resolved} />
      </section>

      <section className="overview-section">
        <div className="overview-overline">エネルギー / ENERGY CURVE</div>
        <EnergyCurve tracks={resolved} />
      </section>
    </aside>
  );
}

function Stat({
  label,
  labelEn,
  value,
}: {
  label: string;
  labelEn: string;
  value: string;
}) {
  return (
    <div className="overview-stat">
      <div className="overview-stat-label">
        {label} / {labelEn}
      </div>
      <div className="overview-stat-value tabular">{value}</div>
    </div>
  );
}

function BpmCurve({ tracks }: { tracks: ResolvedTrack[] }) {
  const known = tracks.filter((t) => t.bpm > 0);
  const h = 80;
  const w = 280;
  if (known.length === 0) {
    return <p className="hint">No BPM data.</p>;
  }
  const minB = Math.min(...known.map((t) => t.bpm)) - 1;
  const maxB = Math.max(...known.map((t) => t.bpm)) + 1;
  const range = maxB - minB || 1;
  const points = known.map((t, i) => {
    const x = known.length === 1 ? w / 2 : (i / (known.length - 1)) * w;
    const y = h - ((t.bpm - minB) / range) * (h - 16) - 8;
    return { x, y, bpm: t.bpm };
  });
  const path = points
    .map((p, i) => (i === 0 ? "M" : "L") + ` ${p.x} ${p.y}`)
    .join(" ");
  return (
    <svg width={w} height={h + 20} style={{ marginTop: 8 }}>
      {[0, 0.5, 1].map((f) => (
        <line
          key={f}
          x1={0}
          y1={h * f + 4}
          x2={w}
          y2={h * f + 4}
          stroke="var(--c-ink-5)"
          strokeWidth={1}
          opacity={0.4}
        />
      ))}
      <path
        d={path + ` L ${w} ${h + 4} L 0 ${h + 4} Z`}
        fill="var(--c-accent)"
        opacity={0.1}
      />
      <path d={path} stroke="var(--c-accent)" strokeWidth={1.5} fill="none" />
      {points.map((p, i) => (
        <g key={i}>
          <circle cx={p.x} cy={p.y} r={3} fill="var(--c-accent)" />
          <text
            x={p.x}
            y={h + 16}
            fill="var(--c-ink-9)"
            fontSize={9}
            textAnchor="middle"
            fontFamily="var(--font-mono)"
          >
            {p.bpm.toFixed(0)}
          </text>
        </g>
      ))}
    </svg>
  );
}

// Camelot wheel: outer ring = A (minor), inner ring = B (major).
// 1〜12 を時計回りに 30° 刻みで配置。
function CamelotWheel({ tracks }: { tracks: ResolvedTrack[] }) {
  const size = 200;
  const cx = size / 2;
  const cy = size / 2;
  const rOuter = 84;
  const rInner = 56;

  const allKeys: { k: string; outer: boolean; angle: number }[] = [];
  for (let i = 1; i <= 12; i++)
    allKeys.push({ k: `${i}A`, outer: true, angle: (i - 1) * 30 - 90 });
  for (let i = 1; i <= 12; i++)
    allKeys.push({ k: `${i}B`, outer: false, angle: (i - 1) * 30 - 90 });

  const posFor = (key: string) => {
    const entry = allKeys.find((x) => x.k === key);
    if (!entry) return null;
    const r = entry.outer ? rOuter : rInner;
    const rad = (entry.angle * Math.PI) / 180;
    return { x: cx + Math.cos(rad) * r, y: cy + Math.sin(rad) * r };
  };

  const positions = tracks
    .map((t) => {
      const p = posFor(t.key);
      return p ? { ...p, key: t.key } : null;
    })
    .filter((p): p is { x: number; y: number; key: string } => p != null);

  return (
    <svg
      width={size}
      height={size}
      style={{ display: "block", margin: "8px auto 0" }}
    >
      <circle
        cx={cx}
        cy={cy}
        r={rOuter + 12}
        fill="var(--c-ink-3)"
        stroke="var(--c-ink-5)"
      />
      <circle
        cx={cx}
        cy={cy}
        r={rInner - 12}
        fill="var(--c-ink-1)"
        stroke="var(--c-ink-5)"
      />
      {allKeys.map((k) => {
        const r = k.outer ? rOuter : rInner;
        const rad = (k.angle * Math.PI) / 180;
        const x = cx + Math.cos(rad) * r;
        const y = cy + Math.sin(rad) * r;
        return (
          <text
            key={k.k}
            x={x}
            y={y + 3}
            fill="var(--c-ink-7)"
            fontSize={8}
            fontFamily="var(--font-mono)"
            textAnchor="middle"
            fontWeight={700}
          >
            {k.k}
          </text>
        );
      })}
      {positions.length > 1 && (
        <polyline
          points={positions.map((p) => `${p.x},${p.y}`).join(" ")}
          stroke="var(--c-accent)"
          strokeWidth={1.5}
          fill="none"
          strokeLinejoin="round"
          strokeDasharray="3 3"
          opacity={0.8}
        />
      )}
      {positions.map((p, i) => (
        <circle
          key={i}
          cx={p.x}
          cy={p.y}
          r={i === 0 ? 5 : 4}
          fill={i === 0 ? "var(--c-accent)" : "var(--c-ink-9)"}
          stroke="var(--c-ink-1)"
          strokeWidth={1.5}
        />
      ))}
      <text
        x={cx}
        y={cy + 3}
        fill="var(--c-ink-9)"
        fontSize={10}
        textAnchor="middle"
        fontWeight={700}
      >
        {tracks.length}
      </text>
    </svg>
  );
}

function EnergyCurve({ tracks }: { tracks: ResolvedTrack[] }) {
  const h = 80;
  const w = 280;
  if (tracks.length === 0) return <p className="hint">No tracks.</p>;
  const points = tracks.map((t, i) => {
    const x = tracks.length === 1 ? w / 2 : (i / (tracks.length - 1)) * w;
    const e = Math.max(0.05, Math.min(1, t.energy || 0.5));
    const y = h - e * (h - 8) - 4;
    return { x, y };
  });
  const path = points
    .map((p, i) => (i === 0 ? "M" : "L") + ` ${p.x} ${p.y}`)
    .join(" ");
  return (
    <svg width={w} height={h + 8} style={{ marginTop: 8 }}>
      <defs>
        <linearGradient id="enGrad" x1={0} y1={0} x2={1} y2={0}>
          <stop offset="0" stopColor="var(--c-accent)" />
          <stop offset="1" stopColor="var(--c-ink-9)" />
        </linearGradient>
      </defs>
      <path
        d={path + ` L ${w} ${h + 4} L 0 ${h + 4} Z`}
        fill="url(#enGrad)"
        opacity={0.15}
      />
      <path d={path} stroke="url(#enGrad)" strokeWidth={1.5} fill="none" />
      {points.map((p, i) => (
        <circle key={i} cx={p.x} cy={p.y} r={3} fill="var(--c-accent)" />
      ))}
    </svg>
  );
}
