import React, { useEffect, useRef } from "react";

import type { WaveformPreview } from "@/types/waveform";

interface WaveformViewProps {
  waveform: WaveformPreview | null;
  /** 0..1。再生位置を波形上に縦線で表示する。 */
  positionRatio: number;
  /** ダウンビート位置を控えめに重ねる（0..1 比率の配列）。 */
  downbeatRatios?: number[];
  /** Hot Cue を比率 (0..1) で重ねる。 */
  hotCueRatios?: { slot: number; ratio: number }[];
  /** クリック時、その x 位置の比率（0..1）が渡される。 */
  onSeekRatio?: (ratio: number) => void;
  height?: number;
}

const HOTCUE_COLORS = [
  "#48E0F4", // 1 cyan
  "#FFC547", // 2 yellow
  "#7AE655", // 3 green
  "#E84A5C", // 4 red
  "#FF7A33", // 5 orange
  "#A98AFF", // 6 violet
  "#FF6BB1", // 7 pink
  "#3E96FF", // 8 blue
];

/**
 * rekordbox 風の 3 バンド波形プレビュー。
 *
 * 各ビンは支配的なバンドで色付け：
 * - low 優勢   → cue gold（`--c-cue`）
 * - mid 優勢   → mint（`--c-accent`）
 * - high 優勢  → info blue（`--c-info`）
 *
 * 再生位置は中央に白の縦線。
 */
export function WaveformView({
  waveform,
  positionRatio,
  downbeatRatios,
  hotCueRatios,
  onSeekRatio,
  height = 80,
}: WaveformViewProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  function handleClick(e: React.MouseEvent<HTMLCanvasElement>) {
    if (!onSeekRatio) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const ratio = (e.clientX - rect.left) / rect.width;
    onSeekRatio(Math.max(0, Math.min(1, ratio)));
  }

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const dpr = window.devicePixelRatio || 1;
    const cssW = canvas.clientWidth || 800;
    canvas.width = Math.floor(cssW * dpr);
    canvas.height = Math.floor(height * dpr);
    canvas.style.height = `${height}px`;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, cssW, height);

    if (!waveform || waveform.sample_count === 0) {
      drawPlaceholder(ctx, cssW, height);
    } else {
      drawWaveform(ctx, waveform, cssW, height);
    }

    if (downbeatRatios && downbeatRatios.length > 0) {
      drawDownbeats(ctx, downbeatRatios, cssW, height);
    }
    if (hotCueRatios && hotCueRatios.length > 0) {
      drawHotCues(ctx, hotCueRatios, cssW, height);
    }

    drawCursor(ctx, positionRatio, cssW, height);
  }, [waveform, positionRatio, downbeatRatios, hotCueRatios, height]);

  return (
    <canvas
      ref={canvasRef}
      className="waveform-canvas"
      onClick={handleClick}
      style={onSeekRatio ? { cursor: "crosshair" } : undefined}
    />
  );
}

function drawDownbeats(
  ctx: CanvasRenderingContext2D,
  ratios: number[],
  width: number,
  height: number,
) {
  // overview では密集を避けるためダウンビートのみ。上下に赤の小リボン。
  ctx.fillStyle = "rgba(255, 45, 85, 0.75)"; // --c-live
  for (const r of ratios) {
    if (r < 0 || r > 1) continue;
    const x = Math.round(r * width);
    ctx.fillRect(x - 1, 0, 2, 5);
    ctx.fillRect(x - 1, height - 5, 2, 5);
  }
}

/** Hot Cue を上端に色付きフラグ + 縦線で表示。 */
function drawHotCues(
  ctx: CanvasRenderingContext2D,
  cues: { slot: number; ratio: number }[],
  width: number,
  height: number,
) {
  for (const c of cues) {
    if (c.ratio < 0 || c.ratio > 1) continue;
    const x = Math.round(c.ratio * width);
    const color = HOTCUE_COLORS[(c.slot - 1) % HOTCUE_COLORS.length]!;

    // 上端の旗（小さな矩形にスロット番号）
    const flagW = 12;
    const flagH = 9;
    ctx.fillStyle = color;
    ctx.fillRect(x, 0, flagW, flagH);
    // 番号
    ctx.fillStyle = "rgba(0, 0, 0, 0.85)";
    ctx.font = "bold 8px JetBrains Mono, monospace";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(String(c.slot), x + flagW / 2, flagH / 2 + 0.5);

    // 細い縦線
    ctx.strokeStyle = `${color}A0`; // 約63% alpha
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(x + 0.5, flagH);
    ctx.lineTo(x + 0.5, height);
    ctx.stroke();

    // 下端のtick
    ctx.fillStyle = color;
    ctx.fillRect(x - 1, height - 4, 3, 4);
  }
}

function drawPlaceholder(ctx: CanvasRenderingContext2D, w: number, h: number) {
  ctx.strokeStyle = "rgba(255, 255, 255, 0.06)";
  ctx.lineWidth = 1;
  ctx.setLineDash([4, 4]);
  ctx.beginPath();
  ctx.moveTo(0, h / 2);
  ctx.lineTo(w, h / 2);
  ctx.stroke();
  ctx.setLineDash([]);
}

function drawWaveform(
  ctx: CanvasRenderingContext2D,
  wf: WaveformPreview,
  width: number,
  height: number,
) {
  const total = wf.sample_count;
  if (total === 0) return;

  // 1 サンプルあたりのピクセル幅（< 1px なら fillRect で合成される）
  const binWidth = width / total;
  const drawWidth = Math.max(binWidth, 1);
  const halfH = height / 2;

  // 色は colors_and_type.css の値と一致させる。
  const COLOR_LOW = "rgba(232, 184, 104, 0.95)"; // --c-cue
  const COLOR_MID = "rgba(79, 227, 178, 0.95)"; // --c-accent
  const COLOR_HIGH = "rgba(62, 168, 255, 0.95)"; // --c-info

  for (let i = 0; i < total; i++) {
    const lo = wf.low[i] ?? 0;
    const mi = wf.mid[i] ?? 0;
    const hi = wf.high[i] ?? 0;
    const peak = Math.max(lo, mi, hi);
    if (peak < 0.005) continue;

    let color: string;
    if (lo >= mi && lo >= hi) color = COLOR_LOW;
    else if (hi >= mi) color = COLOR_HIGH;
    else color = COLOR_MID;

    const barH = peak * halfH;
    ctx.fillStyle = color;
    ctx.fillRect(i * binWidth, halfH - barH, drawWidth, barH * 2);
  }

  // ベースライン
  ctx.strokeStyle = "rgba(255, 255, 255, 0.04)";
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(0, halfH);
  ctx.lineTo(width, halfH);
  ctx.stroke();
}

function drawCursor(
  ctx: CanvasRenderingContext2D,
  ratio: number,
  width: number,
  height: number,
) {
  const x = Math.max(0, Math.min(1, ratio)) * width;
  ctx.strokeStyle = "rgba(255, 255, 255, 0.95)";
  ctx.lineWidth = 1.5;
  ctx.beginPath();
  ctx.moveTo(x + 0.5, 0);
  ctx.lineTo(x + 0.5, height);
  ctx.stroke();

  // 上端に小さな三角インジケータ
  ctx.fillStyle = "rgba(255, 255, 255, 0.95)";
  ctx.beginPath();
  ctx.moveTo(x - 4, 0);
  ctx.lineTo(x + 4, 0);
  ctx.lineTo(x, 5);
  ctx.closePath();
  ctx.fill();
}
