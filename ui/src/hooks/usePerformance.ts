import { useCallback, useEffect, useState } from "react";
import { performanceIpc } from "@/lib/performance";
import type { PerformanceStatus } from "@/types/performance";

/** Network configuration is slower-changing than the shared audio snapshot. No overlapping polls. */
export function usePerformance() {
  const [status, setStatus] = useState<PerformanceStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  const refresh = useCallback(() => setRevision((value) => value + 1), []);
  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const poll = async () => {
      try {
        const next = await performanceIpc.status();
        if (!cancelled) { setStatus(next); setError(null); }
      } catch (reason) {
        if (!cancelled) { setError(String(reason)); setStatus(null); }
      }
      if (!cancelled) timer = setTimeout(() => { void poll(); }, 500);
    };
    void poll();
    return () => { cancelled = true; clearTimeout(timer); };
  }, [revision]);
  return { status, error, refresh };
}
