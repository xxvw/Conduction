import { useEffect, useState } from "react";

import { ipc, type MatchCandidate } from "@/lib/ipc";

interface Args {
  bpm: number;
  keyCamelot: string;
  energy: number;
  excludeTrackId: string | null;
  enabled: boolean;
  limit?: number;
  /** ms。デフォルト 1500ms (要件 §6.5「再生中にリアルタイム更新」)。 */
  intervalMs?: number;
}

/** アクティブデッキの状態に対する Cue 候補を 1.5Hz で polling する。 */
export function useMatchCandidates({
  bpm,
  keyCamelot,
  energy,
  excludeTrackId,
  enabled,
  limit = 6,
  intervalMs = 1500,
}: Args): MatchCandidate[] {
  const [candidates, setCandidates] = useState<MatchCandidate[]>([]);

  useEffect(() => {
    if (!enabled || bpm <= 0 || !keyCamelot) {
      setCandidates([]);
      return;
    }
    let cancelled = false;
    const fetchOnce = () => {
      ipc
        .listMatchCandidates({
          bpm,
          key_camelot: keyCamelot,
          energy,
          exclude_track_id: excludeTrackId ?? undefined,
          limit,
        })
        .then((r) => {
          if (!cancelled) setCandidates(r);
        })
        .catch(() => {
          if (!cancelled) setCandidates([]);
        });
    };
    fetchOnce();
    const id = window.setInterval(fetchOnce, intervalMs);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [bpm, keyCamelot, energy, excludeTrackId, enabled, limit, intervalMs]);

  return candidates;
}
