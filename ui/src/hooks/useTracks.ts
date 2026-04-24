import { useCallback, useEffect, useState } from "react";

import { ipc } from "@/lib/ipc";
import type { TrackSummary } from "@/types/track";

export function useTracks() {
  const [tracks, setTracks] = useState<TrackSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const list = await ipc.listTracks();
      setTracks(list);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { tracks, loading, error, refresh };
}
