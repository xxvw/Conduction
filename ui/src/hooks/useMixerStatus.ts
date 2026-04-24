import { useEffect, useState } from "react";

import { ipc } from "@/lib/ipc";
import type { MixerSnapshot } from "@/types/mixer";

/**
 * Mixer のスナップショットを一定周期でポーリングする。
 * intervalMs が極端に短いと IPC 往復が詰まるため、100ms 前後を推奨。
 */
export function useMixerStatus(intervalMs = 100): MixerSnapshot | null {
  const [snapshot, setSnapshot] = useState<MixerSnapshot | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function tick() {
      try {
        const snap = await ipc.getStatus();
        if (!cancelled) setSnapshot(snap);
      } catch {
        // 起動直後やシャットダウン中は握りつぶす
      }
    }

    void tick();
    const id = window.setInterval(tick, intervalMs);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [intervalMs]);

  return snapshot;
}
