import { useCallback, useEffect, useState } from "react";

import { ipc, type CueDto, type InsertCueArgs } from "@/lib/ipc";

export interface UseCuesResult {
  cues: CueDto[];
  refresh: () => Promise<void>;
  insert: (args: InsertCueArgs) => Promise<CueDto | null>;
  remove: (cueId: string) => Promise<void>;
}

/** トラックの Typed Cue (intro/drop/breakdown 等) を取得・編集するフック。 */
export function useCues(trackId: string | null): UseCuesResult {
  const [cues, setCues] = useState<CueDto[]>([]);

  const refresh = useCallback(async () => {
    if (!trackId) {
      setCues([]);
      return;
    }
    try {
      const r = await ipc.listCues(trackId);
      setCues(r);
    } catch {
      setCues([]);
    }
  }, [trackId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const insert = useCallback(
    async (args: InsertCueArgs): Promise<CueDto | null> => {
      try {
        const created = await ipc.insertCue(args);
        await refresh();
        return created;
      } catch (e) {
        // eslint-disable-next-line no-console
        console.error("insertCue failed", e);
        return null;
      }
    },
    [refresh],
  );

  const remove = useCallback(
    async (cueId: string) => {
      try {
        await ipc.deleteCue(cueId);
        await refresh();
      } catch (e) {
        // eslint-disable-next-line no-console
        console.error("deleteCue failed", e);
      }
    },
    [refresh],
  );

  return { cues, refresh, insert, remove };
}
