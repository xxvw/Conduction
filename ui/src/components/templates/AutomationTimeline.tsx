import { useEffect, useRef } from "react";

import type {
  AutomationTrack,
  BuiltInTarget,
  CurveType,
  TemplateFull,
  TimePosition,
} from "@/lib/ipc";

interface AutomationTimelineProps {
  template: TemplateFull;
  height?: number;
}

const TRACK_ROW_HEIGHT = 72;
const TRACK_LABEL_WIDTH = 160;
const X_AXIS_HEIGHT = 22;

export function AutomationTimeline({ template, height }: AutomationTimelineProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  const computedHeight =
    height ?? X_AXIS_HEIGHT + Math.max(1, template.tracks.length) * TRACK_ROW_HEIGHT + 8;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const dpr = window.devicePixelRatio || 1;
    const cssW = canvas.clientWidth || 1000;
    canvas.width = Math.floor(cssW * dpr);
    canvas.height = Math.floor(computedHeight * dpr);
    canvas.style.height = `${computedHeight}px`;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, cssW, computedHeight);

    drawTimeline(ctx, cssW, computedHeight, template);
  }, [template, computedHeight]);

  return (
    <canvas
      ref={canvasRef}
      className="automation-timeline"
      aria-label={`Automation tracks for ${template.name}`}
    />
  );
}

// ---------- drawing helpers ----------

function drawTimeline(
  ctx: CanvasRenderingContext2D,
  width: number,
  height: number,
  template: TemplateFull,
) {
  const trackArea = {
    x: TRACK_LABEL_WIDTH,
    y: X_AXIS_HEIGHT,
    w: width - TRACK_LABEL_WIDTH - 4,
    h: height - X_AXIS_HEIGHT - 8,
  };

  drawXAxis(ctx, template.duration_beats, trackArea, width);
  drawGridV(ctx, template.duration_beats, trackArea);

  const tracks = template.tracks;
  for (let i = 0; i < tracks.length; i++) {
    const trackTop = trackArea.y + i * TRACK_ROW_HEIGHT;
    drawTrack(ctx, tracks[i]!, template.duration_beats, {
      x: trackArea.x,
      y: trackTop,
      w: trackArea.w,
      h: TRACK_ROW_HEIGHT,
    });
  }
}

function drawXAxis(
  ctx: CanvasRenderingContext2D,
  totalBeats: number,
  area: { x: number; y: number; w: number; h: number },
  fullWidth: number,
) {
  ctx.fillStyle = "rgba(13,15,20,0.85)";
  ctx.fillRect(0, 0, fullWidth, X_AXIS_HEIGHT);
  ctx.strokeStyle = "rgba(255,255,255,0.06)";
  ctx.beginPath();
  ctx.moveTo(0, X_AXIS_HEIGHT - 0.5);
  ctx.lineTo(fullWidth, X_AXIS_HEIGHT - 0.5);
  ctx.stroke();

  ctx.fillStyle = "rgba(255,255,255,0.55)";
  ctx.font = "10px JetBrains Mono, monospace";
  ctx.textBaseline = "middle";

  // 4-beat (= 1 bar) ごとに目盛り。8 拍以下なら全拍を出す。
  const stepBeats = totalBeats <= 16 ? 4 : Math.max(4, Math.round(totalBeats / 16 / 4) * 4);
  for (let b = 0; b <= totalBeats; b += stepBeats) {
    const x = area.x + (b / totalBeats) * area.w;
    ctx.textAlign = b === 0 ? "left" : b === totalBeats ? "right" : "center";
    const bar = Math.floor(b / 4);
    const label = b === 0 ? "0" : `${bar}b`;
    ctx.fillText(label, x, X_AXIS_HEIGHT / 2);
  }
}

function drawGridV(
  ctx: CanvasRenderingContext2D,
  totalBeats: number,
  area: { x: number; y: number; w: number; h: number },
) {
  ctx.strokeStyle = "rgba(255,255,255,0.04)";
  ctx.lineWidth = 1;
  const beatPx = area.w / totalBeats;
  // 4 拍ごとに薄い線、16 拍ごとに少し濃い線
  for (let b = 0; b <= totalBeats; b += 4) {
    const x = area.x + b * beatPx;
    ctx.strokeStyle =
      b % 16 === 0 ? "rgba(255,255,255,0.10)" : "rgba(255,255,255,0.04)";
    ctx.beginPath();
    ctx.moveTo(x + 0.5, area.y);
    ctx.lineTo(x + 0.5, area.y + area.h);
    ctx.stroke();
  }
}

function drawTrack(
  ctx: CanvasRenderingContext2D,
  track: AutomationTrack,
  totalBeats: number,
  area: { x: number; y: number; w: number; h: number },
) {
  const label = targetLabel(track.target);
  const color = trackColor(track.target);
  const { min, max } = valueRange(track.target);

  // ラベル領域
  ctx.fillStyle = color;
  ctx.font = "bold 11px var(--font-mono, JetBrains Mono, monospace)";
  ctx.textAlign = "left";
  ctx.textBaseline = "top";
  ctx.fillText(label, 8, area.y + 6);

  ctx.fillStyle = "rgba(255,255,255,0.5)";
  ctx.font = "9px JetBrains Mono, monospace";
  ctx.fillText(`${formatValue(min)} … ${formatValue(max)}`, 8, area.y + 22);

  // 行の上下ライン
  ctx.strokeStyle = "rgba(255,255,255,0.04)";
  ctx.beginPath();
  ctx.moveTo(0, area.y + area.h + 0.5);
  ctx.lineTo(area.x + area.w, area.y + area.h + 0.5);
  ctx.stroke();

  // 値→y 変換
  const padY = 8;
  const valueToY = (v: number) => {
    const t = (v - min) / (max - min);
    return area.y + padY + (1 - t) * (area.h - padY * 2);
  };
  const beatToX = (b: number) =>
    area.x + Math.max(0, Math.min(1, b / totalBeats)) * area.w;

  // 0 ライン (range が ±方向のとき)
  if (min < 0 && max > 0) {
    const y0 = valueToY(0);
    ctx.strokeStyle = "rgba(255,255,255,0.08)";
    ctx.setLineDash([3, 3]);
    ctx.beginPath();
    ctx.moveTo(area.x, y0 + 0.5);
    ctx.lineTo(area.x + area.w, y0 + 0.5);
    ctx.stroke();
    ctx.setLineDash([]);
  }

  // 曲線描画 (サンプル取って polyline)
  const sortedKfs = [...track.keyframes]
    .map((kf) => ({
      beat: positionToBeats(kf.position, totalBeats),
      value: kf.value,
      curve: kf.curve,
    }))
    .sort((a, b) => a.beat - b.beat);
  if (sortedKfs.length === 0) return;

  ctx.strokeStyle = color;
  ctx.lineWidth = 1.6;
  ctx.beginPath();
  const samples = 200;
  for (let i = 0; i <= samples; i++) {
    const beat = (i / samples) * totalBeats;
    const v = evaluateTrackAtBeat(sortedKfs, beat);
    const x = beatToX(beat);
    const y = valueToY(v);
    if (i === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  }
  ctx.stroke();

  // Keyframe マーカー
  for (const kf of sortedKfs) {
    const x = beatToX(kf.beat);
    const y = valueToY(kf.value);
    ctx.fillStyle = color;
    ctx.beginPath();
    ctx.arc(x, y, 3.5, 0, Math.PI * 2);
    ctx.fill();
    // 中央に小さな黒い穴 (見やすく)
    ctx.fillStyle = "rgba(0,0,0,0.85)";
    ctx.beginPath();
    ctx.arc(x, y, 1.4, 0, Math.PI * 2);
    ctx.fill();
  }
}

// ---------- evaluation (UI 側の最小実装、backend の evaluate_track と同じロジック) ----------

interface NormalizedKf {
  beat: number;
  value: number;
  curve: CurveType;
}

function evaluateTrackAtBeat(kfs: NormalizedKf[], beat: number): number {
  if (kfs.length === 0) return 0;
  if (beat <= kfs[0]!.beat) return kfs[0]!.value;
  if (beat >= kfs[kfs.length - 1]!.beat) return kfs[kfs.length - 1]!.value;

  let prev = kfs[0]!;
  let next = kfs[kfs.length - 1]!;
  for (let i = 0; i < kfs.length - 1; i++) {
    if (kfs[i]!.beat <= beat && beat <= kfs[i + 1]!.beat) {
      prev = kfs[i]!;
      next = kfs[i + 1]!;
      break;
    }
  }
  const span = next.beat - prev.beat;
  if (span <= 1e-9) return prev.value;
  if (prev.curve === "step" || prev.curve === "hold") return prev.value;
  const t = (beat - prev.beat) / span;
  return prev.value + (next.value - prev.value) * ease(t, prev.curve);
}

function ease(t: number, curve: CurveType): number {
  t = Math.max(0, Math.min(1, t));
  switch (curve) {
    case "linear":
      return t;
    case "ease_in":
      return t * t;
    case "ease_out":
      return 1 - (1 - t) * (1 - t);
    case "ease_in_out":
      return t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;
    case "step":
    case "hold":
      return 0;
  }
}

function positionToBeats(pos: TimePosition, totalBeats: number): number {
  switch (pos.kind) {
    case "beats":
      return pos.value;
    case "beats_from_end":
      return totalBeats - pos.value;
    case "seconds":
      // テンプレートエディタでは固定 128 BPM で描画。実行時は実 BPM で再評価される。
      return (pos.value * 128) / 60;
  }
}

// ---------- target metadata ----------

function targetLabel(target: BuiltInTarget): string {
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

function valueRange(target: BuiltInTarget): { min: number; max: number } {
  switch (target.type) {
    case "crossfader":
    case "deck_filter":
      return { min: -1, max: 1 };
    case "master_volume":
    case "deck_volume":
      return { min: 0, max: 2 };
    case "deck_eq_low":
    case "deck_eq_mid":
    case "deck_eq_high":
      return { min: -26, max: 6 };
    case "deck_echo_wet":
    case "deck_reverb_wet":
      return { min: 0, max: 1 };
  }
}

function trackColor(target: BuiltInTarget): string {
  switch (target.type) {
    case "crossfader":
      return "#4FE3B2";
    case "master_volume":
      return "#FFC547";
    case "deck_volume":
      return target.deck === "A" ? "#4FE3B2" : "#E8915A";
    case "deck_eq_low":
    case "deck_eq_mid":
    case "deck_eq_high":
      return target.deck === "A" ? "#7AE655" : "#FF7A33";
    case "deck_filter":
      return "#8A9BE8";
    case "deck_echo_wet":
      return "#A089DC";
    case "deck_reverb_wet":
      return "#48E0F4";
  }
}

function formatValue(v: number): string {
  if (Math.abs(v) >= 10) return v.toFixed(0);
  if (Math.abs(v - Math.round(v)) < 1e-6) return v.toFixed(0);
  return v.toFixed(1);
}
