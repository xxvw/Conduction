import { useCallback, useEffect, useState } from "react";

import { ipc } from "@/lib/ipc";
import type { HotCueDto } from "@/types/hotcue";

export interface UseHotCuesResult {
  cues: HotCueDto[];
  /** slot 1..=8。位置（秒）を保存（既にあれば上書き）。 */
  set: (slot: number, positionSec: number) => Promise<void>;
  /** slot 1..=8 を削除。 */
  remove: (slot: number) => Promise<void>;
  /** 該当 slot の Hot Cue（存在しなければ null） */
  get: (slot: number) => HotCueDto | null;
}

/** 指定 track の Hot Cue を読み書きする。 */
export function useHotCues(trackId: string | null): UseHotCuesResult {
  const [cues, setCues] = useState<HotCueDto[]>([]);

  const refresh = useCallback(async () => {
    if (!trackId) {
      setCues([]);
      return;
    }
    try {
      const list = await ipc.listHotCues(trackId);
      setCues(list);
    } catch {
      // ignore
    }
  }, [trackId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const set = useCallback(
    async (slot: number, positionSec: number) => {
      if (!trackId) return;
      try {
        await ipc.setHotCue(trackId, slot, positionSec);
        await refresh();
      } catch (e) {
        console.error("setHotCue failed:", e);
      }
    },
    [trackId, refresh],
  );

  const remove = useCallback(
    async (slot: number) => {
      if (!trackId) return;
      try {
        await ipc.deleteHotCue(trackId, slot);
        await refresh();
      } catch (e) {
        console.error("deleteHotCue failed:", e);
      }
    },
    [trackId, refresh],
  );

  const get = useCallback(
    (slot: number) => cues.find((c) => c.slot === slot) ?? null,
    [cues],
  );

  return { cues, set, remove, get };
}
