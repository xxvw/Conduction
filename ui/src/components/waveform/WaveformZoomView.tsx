import React, { useEffect, useRef } from "react";

import type { BeatDto } from "@/types/beat";
import type { WaveformPreview } from "@/types/waveform";

interface WaveformZoomViewProps {
  waveform: WaveformPreview | null;
  beats: BeatDto[];
  /** 再生位置（秒）。中央に固定される。 */
  positionSec: number;
  /** 楽曲全長（秒）。 */
  durationSec: number;
  /** 波形が再生位置の左右に表示する半幅（秒）。 */
  windowSec?: number;
  height?: number;
  /** クリックした時、その秒位置にシークするためのコールバック。 */
  onSeekSec?: (sec: number) => void;
}

/**
 * rekordbox の下段に相当する詳細波形。再生位置を中央に固定し、
 * ±windowSec の範囲を拡大して表示。ビートライン（縦線）を重ねる。
 *
 * 詳細用の高密度波形データはまだ持っていないため、overview の bin を
 * 時間スケールでマッピングして拡大表示する。
 */
export function WaveformZoomView({
  waveform,
  beats,
  positionSec,
  durationSec,
  windowSec = 4,
  height = 64,
  onSeekSec,
}: WaveformZoomViewProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  function handleClick(e: React.MouseEvent<HTMLCanvasElement>) {
    if (!onSeekSec) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const ratio = (e.clientX - rect.left) / rect.width;
    const startSec = positionSec - windowSec;
    const target = startSec + ratio * windowSec * 2;
    onSeekSec(Math.max(0, target));
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

    const startSec = positionSec - windowSec;
    const endSec = positionSec + windowSec;
    const span = windowSec * 2;
    const xOf = (sec: number) => ((sec - startSec) / span) * cssW;

    drawZoomBackground(ctx, cssW, height);
    if (waveform && waveform.sample_count > 0 && durationSec > 0) {
      drawZoomWaveform(ctx, waveform, durationSec, startSec, span, cssW, height);
    }
    drawBeatLines(ctx, beats, startSec, endSec, xOf, height);
    drawCenterCursor(ctx, cssW, height);
  }, [waveform, beats, positionSec, durationSec, windowSec, height]);

  return (
    <canvas
      ref={canvasRef}
      className="waveform-zoom-canvas"
      onClick={handleClick}
      style={onSeekSec ? { cursor: "crosshair" } : undefined}
    />
  );
}

function drawZoomBackground(
  ctx: CanvasRenderingContext2D,
  width: number,
  height: number,
) {
  // ベースライン
  ctx.strokeStyle = "rgba(255, 255, 255, 0.04)";
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(0, height / 2);
  ctx.lineTo(width, height / 2);
  ctx.stroke();
}

function drawZoomWaveform(
  ctx: CanvasRenderingContext2D,
  wf: WaveformPreview,
  durationSec: number,
  startSec: number,
  span: number,
  width: number,
  height: number,
) {
  const total = wf.sample_count;
  const secPerBin = durationSec / total;
  const halfH = height / 2;

  const COLOR_LOW = "rgba(232, 184, 104, 0.95)";
  const COLOR_MID = "rgba(79, 227, 178, 0.95)";
  const COLOR_HIGH = "rgba(62, 168, 255, 0.95)";

  const binStart = Math.max(0, Math.floor(startSec / secPerBin));
  const binEnd = Math.min(total, Math.ceil((startSec + span) / secPerBin));

  for (let i = binStart; i < binEnd; i++) {
    const lo = wf.low[i] ?? 0;
    const mi = wf.mid[i] ?? 0;
    const hi = wf.high[i] ?? 0;
    const peak = Math.max(lo, mi, hi);
    if (peak < 0.005) continue;

    const tStart = i * secPerBin;
    const tEnd = tStart + secPerBin;
    const xStart = ((tStart - startSec) / span) * width;
    const xEnd = ((tEnd - startSec) / span) * width;
    const w = Math.max(xEnd - xStart, 1);

    let color: string;
    if (lo >= mi && lo >= hi) color = COLOR_LOW;
    else if (hi >= mi) color = COLOR_HIGH;
    else color = COLOR_MID;

    const barH = peak * halfH;
    ctx.fillStyle = color;
    ctx.fillRect(xStart, halfH - barH, w, barH * 2);
  }
}

function drawBeatLines(
  ctx: CanvasRenderingContext2D,
  beats: BeatDto[],
  startSec: number,
  endSec: number,
  xOf: (sec: number) => number,
  height: number,
) {
  // 上下端に矩形マーカー：通常拍は白、ダウンビート（毎4拍）は赤。
  // 縦線は控えめに（カーソルや波形を邪魔しない程度）。
  const COLOR_DOWN = "rgba(255, 45, 85, 0.95)"; // --c-live
  const COLOR_BEAT = "rgba(255, 255, 255, 0.85)";

  for (const beat of beats) {
    if (beat.position_sec < startSec || beat.position_sec > endSec) continue;
    const x = Math.round(xOf(beat.position_sec));

    if (beat.is_downbeat) {
      // 縦線（控えめな赤）
      ctx.strokeStyle = "rgba(255, 45, 85, 0.30)";
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(x + 0.5, 0);
      ctx.lineTo(x + 0.5, height);
      ctx.stroke();

      // 上下端の矩形マーカー（赤、太め）
      ctx.fillStyle = COLOR_DOWN;
      const w = 4;
      const h = 7;
      ctx.fillRect(x - w / 2, 0, w, h);
      ctx.fillRect(x - w / 2, height - h, w, h);
    } else {
      // 縦線（控えめな白）
      ctx.strokeStyle = "rgba(255, 255, 255, 0.10)";
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(x + 0.5, 0);
      ctx.lineTo(x + 0.5, height);
      ctx.stroke();

      // 上下端の矩形マーカー（白、細め）
      ctx.fillStyle = COLOR_BEAT;
      const w = 2;
      const h = 4;
      ctx.fillRect(x - w / 2, 0, w, h);
      ctx.fillRect(x - w / 2, height - h, w, h);
    }
  }
}

function drawCenterCursor(
  ctx: CanvasRenderingContext2D,
  width: number,
  height: number,
) {
  const x = width / 2;
  // CDJ 風の赤い縦線
  ctx.strokeStyle = "rgba(255, 45, 85, 0.92)";
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.moveTo(x + 0.5, 0);
  ctx.lineTo(x + 0.5, height);
  ctx.stroke();

  // 上端ピンク三角
  ctx.fillStyle = "rgba(255, 45, 85, 0.92)";
  ctx.beginPath();
  ctx.moveTo(x - 5, 0);
  ctx.lineTo(x + 5, 0);
  ctx.lineTo(x, 6);
  ctx.closePath();
  ctx.fill();
}
