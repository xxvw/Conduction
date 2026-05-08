import { useEffect, useState } from "react";

import { ipc } from "@/lib/ipc";
import type { WaveformPreview } from "@/types/waveform";

/**
 * 指定 track の波形を取得し、生成中なら数秒間隔でリトライする。
 *
 * import_track 直後はバックエンドが波形を生成中で `null` が返ることがあるため、
 * 自動でポーリングする。
 */
export function useWaveform(trackId: string | null): WaveformPreview | null {
  const [waveform, setWaveform] = useState<WaveformPreview | null>(null);

  useEffect(() => {
    setWaveform(null);
    if (!trackId) return;

    let cancelled = false;
    let interval: number | null = null;

    const fetchOnce = async () => {
      try {
        const wf = await ipc.getWaveform(trackId);
        if (cancelled) return;
        if (wf) {
          setWaveform(wf);
          if (interval !== null) {
            window.clearInterval(interval);
            interval = null;
          }
        }
      } catch {
        // 起動直後やシャットダウン中は握りつぶす
      }
    };

    void fetchOnce();
    interval = window.setInterval(() => {
      void fetchOnce();
    }, 1500);

    return () => {
      cancelled = true;
      if (interval !== null) window.clearInterval(interval);
    };
  }, [trackId]);

  return waveform;
}
