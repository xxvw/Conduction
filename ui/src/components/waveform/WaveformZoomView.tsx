import React, { useEffect, useRef } from "react";

import type { BeatDto } from "@/types/beat";
import type { HotCueDto } from "@/types/hotcue";
import type { WaveformPreview } from "@/types/waveform";

interface WaveformZoomViewProps {
  waveform: WaveformPreview | null;
  beats: BeatDto[];
  hotCues?: HotCueDto[];
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
 * rekordbox 下段相当の詳細波形。
 *
 * 構造:
 *   ┌─ marker zone (top, 8px) ─────┐  ← 拍マーカーのリボン
 *   │                              │
 *   │  multi-band stacked waveform │  ← low(赤)/mid(緑)/high(青)を中心線から外側へ積む
 *   │                              │
 *   └─ marker zone (bottom, 8px) ──┘
 */
export function WaveformZoomView({
  waveform,
  beats,
  hotCues,
  positionSec,
  durationSec,
  windowSec = 4,
  height = 72,
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

    const markerTop = 8;
    const markerBottom = 8;

    drawMarkerZones(ctx, cssW, height, markerTop, markerBottom);

    if (waveform && waveform.sample_count > 0 && durationSec > 0) {
      drawStackedWaveform(
        ctx,
        waveform,
        durationSec,
        startSec,
        span,
        cssW,
        height,
        markerTop,
        markerBottom,
      );
    }

    drawBeatRibbons(ctx, beats, startSec, endSec, xOf, cssW, height, markerTop, markerBottom);
    if (hotCues && hotCues.length > 0) {
      drawHotCues(ctx, hotCues, startSec, endSec, xOf, height, markerTop, markerBottom);
    }
    drawCenterCursor(ctx, cssW, height);
  }, [waveform, beats, hotCues, positionSec, durationSec, windowSec, height]);

  return (
    <canvas
      ref={canvasRef}
      className="waveform-zoom-canvas"
      onClick={handleClick}
      style={onSeekSec ? { cursor: "crosshair" } : undefined}
    />
  );
}

/** marker zone (上下) に薄いベース色を引く。マーカーの背景。 */
function drawMarkerZones(
  ctx: CanvasRenderingContext2D,
  width: number,
  height: number,
  topH: number,
  bottomH: number,
) {
  ctx.fillStyle = "rgba(255, 255, 255, 0.03)";
  ctx.fillRect(0, 0, width, topH);
  ctx.fillRect(0, height - bottomH, width, bottomH);
}

/** 中心線から low → mid → high の順に外側へ積み重ねる多色波形。 */
function drawStackedWaveform(
  ctx: CanvasRenderingContext2D,
  wf: WaveformPreview,
  durationSec: number,
  startSec: number,
  span: number,
  width: number,
  height: number,
  markerTop: number,
  markerBottom: number,
) {
  const total = wf.sample_count;
  const secPerBin = durationSec / total;

  const innerTop = markerTop;
  const innerBottom = height - markerBottom;
  const innerH = innerBottom - innerTop;
  const halfH = innerH / 2;
  const centerY = innerTop + halfH;

  // 周波数帯ごとの色:
  //   low  → red    (#E84A5C)
  //   mid  → mint   (#4FE3B2)
  //   high → blue   (#3EA8FF)
  const COLOR_LOW = "rgba(232, 74, 92, 0.95)";
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

    // 各バンドの高さ。peakで割らずそのまま [0,1] を halfH にスケール。
    const lowH = lo * halfH;
    const midH = mi * halfH;
    const highH = hi * halfH;

    // 中心 → 上方向: low → mid → high の順で積む（低音は中央寄り、高音は外側）
    let yTop = centerY;
    if (lowH > 0) {
      ctx.fillStyle = COLOR_LOW;
      ctx.fillRect(xStart, yTop - lowH, w, lowH);
      yTop -= lowH;
    }
    if (midH > 0) {
      ctx.fillStyle = COLOR_MID;
      ctx.fillRect(xStart, yTop - midH, w, midH);
      yTop -= midH;
    }
    if (highH > 0) {
      ctx.fillStyle = COLOR_HIGH;
      ctx.fillRect(xStart, yTop - highH, w, highH);
    }

    // 中心 → 下方向: 対称
    let yBottom = centerY;
    if (lowH > 0) {
      ctx.fillStyle = COLOR_LOW;
      ctx.fillRect(xStart, yBottom, w, lowH);
      yBottom += lowH;
    }
    if (midH > 0) {
      ctx.fillStyle = COLOR_MID;
      ctx.fillRect(xStart, yBottom, w, midH);
      yBottom += midH;
    }
    if (highH > 0) {
      ctx.fillStyle = COLOR_HIGH;
      ctx.fillRect(xStart, yBottom, w, highH);
    }
  }
}

/** 拍マーカー: rekordbox 風の上下リボン帯。ダウンビート赤、通常拍白。 */
function drawBeatRibbons(
  ctx: CanvasRenderingContext2D,
  beats: BeatDto[],
  startSec: number,
  endSec: number,
  xOf: (sec: number) => number,
  width: number,
  height: number,
  topH: number,
  bottomH: number,
) {
  const COLOR_DOWN = "rgba(255, 45, 85, 0.98)"; // --c-live
  const COLOR_BEAT = "rgba(255, 255, 255, 0.95)";
  const GUIDE_DOWN = "rgba(255, 45, 85, 0.35)";
  const GUIDE_BEAT = "rgba(255, 255, 255, 0.10)";

  const innerTop = topH;
  const innerBottom = height - bottomH;

  for (const beat of beats) {
    if (beat.position_sec < startSec || beat.position_sec > endSec) continue;
    const x = Math.round(xOf(beat.position_sec));
    if (x < 0 || x > width) continue;

    if (beat.is_downbeat) {
      // 縦線（控えめな赤）
      ctx.strokeStyle = GUIDE_DOWN;
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.moveTo(x + 0.5, innerTop);
      ctx.lineTo(x + 0.5, innerBottom);
      ctx.stroke();

      // 上下リボン（赤、太め）
      ctx.fillStyle = COLOR_DOWN;
      const w = 6;
      ctx.fillRect(x - w / 2, 0, w, topH);
      ctx.fillRect(x - w / 2, innerBottom, w, bottomH);

      // 上端の小さなノッチ
      ctx.fillStyle = COLOR_DOWN;
      ctx.beginPath();
      ctx.moveTo(x - w / 2 - 2, 0);
      ctx.lineTo(x + w / 2 + 2, 0);
      ctx.lineTo(x, topH + 4);
      ctx.closePath();
      ctx.fill();
    } else {
      // 縦線（控えめな白）
      ctx.strokeStyle = GUIDE_BEAT;
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(x + 0.5, innerTop);
      ctx.lineTo(x + 0.5, innerBottom);
      ctx.stroke();

      // 上下リボン（白、細め）
      ctx.fillStyle = COLOR_BEAT;
      const w = 2;
      ctx.fillRect(x - w / 2, 0, w, topH);
      ctx.fillRect(x - w / 2, innerBottom, w, bottomH);
    }
  }
}

/** Hot Cue マーカー: 縦線 + 上端のスロット番号付きフラグ。色はスロットごとに固定。 */
function drawHotCues(
  ctx: CanvasRenderingContext2D,
  hotCues: HotCueDto[],
  startSec: number,
  endSec: number,
  xOf: (sec: number) => number,
  height: number,
  topH: number,
  bottomH: number,
) {
  // rekordbox 互換のスロット色（slot 1..8）
  const SLOT_COLORS = [
    "#48E0F4", // 1 cyan
    "#FFC547", // 2 yellow
    "#7AE655", // 3 green
    "#E84A5C", // 4 red
    "#FF7A33", // 5 orange
    "#A98AFF", // 6 violet
    "#FF6BB1", // 7 pink
    "#3E96FF", // 8 blue
  ];

  for (const c of hotCues) {
    if (c.position_sec < startSec || c.position_sec > endSec) continue;
    const x = Math.round(xOf(c.position_sec));
    const color = SLOT_COLORS[(c.slot - 1) % SLOT_COLORS.length];

    // 縦線
    ctx.strokeStyle = `${color}E0`; // 約88% alpha
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.moveTo(x + 0.5, topH);
    ctx.lineTo(x + 0.5, height - bottomH);
    ctx.stroke();

    // 上端の旗（数字入りの小さな矩形）
    ctx.fillStyle = color;
    const flagW = 16;
    const flagH = topH;
    ctx.fillRect(x, 0, flagW, flagH);
    ctx.fillStyle = "rgba(0, 0, 0, 0.85)";
    ctx.font = "bold 9px JetBrains Mono, monospace";
    ctx.textBaseline = "middle";
    ctx.textAlign = "center";
    ctx.fillText(String(c.slot), x + flagW / 2, flagH / 2 + 0.5);

    // 下端マーカー（同色、細め）
    ctx.fillStyle = color;
    ctx.fillRect(x - 1, height - bottomH, 3, bottomH);
  }
}

function drawCenterCursor(
  ctx: CanvasRenderingContext2D,
  width: number,
  height: number,
) {
  const x = width / 2;
  // CDJ 風の白い縦線（赤は拍マーカー側で使うので変更）
  ctx.strokeStyle = "rgba(255, 255, 255, 0.95)";
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.moveTo(x + 0.5, 0);
  ctx.lineTo(x + 0.5, height);
  ctx.stroke();

  // 上端三角インジケータ
  ctx.fillStyle = "rgba(255, 255, 255, 0.95)";
  ctx.beginPath();
  ctx.moveTo(x - 5, 0);
  ctx.lineTo(x + 5, 0);
  ctx.lineTo(x, 6);
  ctx.closePath();
  ctx.fill();
}
