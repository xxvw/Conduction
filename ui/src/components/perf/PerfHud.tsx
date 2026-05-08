import { useEffect, useRef, useState } from "react";

import { ipc, type ResourceStats } from "@/lib/ipc";

/**
 * 開発用パフォーマンスHUD。
 * - FPS: requestAnimationFrame を1秒窓で集計
 * - CPU / Memory: バックエンドの sysinfo（1秒間隔ポーリング）
 * - GPU: macOS 標準 API では取得困難なので非対応
 */
export function PerfHud() {
  const [fps, setFps] = useState(0);
  const [stats, setStats] = useState<ResourceStats | null>(null);

  // FPS 計測
  useEffect(() => {
    let frames = 0;
    let last = performance.now();
    let raf = 0;
    const tick = () => {
      frames += 1;
      const now = performance.now();
      if (now - last >= 1000) {
        setFps(Math.round((frames * 1000) / (now - last)));
        frames = 0;
        last = now;
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);

  // CPU / Memory ポーリング
  const stopRef = useRef(false);
  useEffect(() => {
    stopRef.current = false;
    let timer = 0;
    const poll = async () => {
      try {
        const s = await ipc.getResourceStats();
        if (!stopRef.current) setStats(s);
      } catch {
        // ignore
      }
      if (!stopRef.current) {
        timer = window.setTimeout(poll, 1000);
      }
    };
    void poll();
    return () => {
      stopRef.current = true;
      if (timer) window.clearTimeout(timer);
    };
  }, []);

  return (
    <div className="perf-hud" role="status" aria-label="performance">
      <Metric label="FPS" value={fps.toString()} warn={fps > 0 && fps < 30} />
      <Metric
        label="CPU"
        value={stats ? `${stats.cpu_percent.toFixed(1)}%` : "—"}
        warn={stats != null && stats.cpu_percent > 200}
      />
      <Metric
        label="MEM"
        value={stats ? `${stats.memory_mb.toFixed(0)}MB` : "—"}
      />
      <Metric label="GPU" value="—" muted />
    </div>
  );
}

function Metric({
  label,
  value,
  warn,
  muted,
}: {
  label: string;
  value: string;
  warn?: boolean;
  muted?: boolean;
}) {
  return (
    <span className="perf-metric" data-warn={warn || undefined} data-muted={muted || undefined}>
      <span className="perf-label">{label}</span>
      <span className="perf-value">{value}</span>
    </span>
  );
}
