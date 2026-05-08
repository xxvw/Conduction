import { useEffect, useState } from "react";

import { ipc } from "@/lib/ipc";
import type { BeatDto } from "@/types/beat";

/**
 * 指定 track のビートグリッドを取得する。
 * 解析中の場合は空配列を返し、polling で再取得する。
 */
export function useBeats(trackId: string | null): BeatDto[] {
  const [beats, setBeats] = useState<BeatDto[]>([]);

  useEffect(() => {
    setBeats([]);
    if (!trackId) return;

    let cancelled = false;
    let interval: number | null = null;

    const fetchOnce = async () => {
      try {
        const list = await ipc.getTrackBeats(trackId);
        if (cancelled) return;
        if (list.length > 0) {
          setBeats(list);
          if (interval !== null) {
            window.clearInterval(interval);
            interval = null;
          }
        }
      } catch {
        // ignore
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

  return beats;
}
