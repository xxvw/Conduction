import { useEffect, useRef, useState } from "react";

import type { DeckSnapshot } from "@/types/mixer";

/**
 * バックエンドの mixer snapshot は 10 Hz でしか届かないので、
 * その間を `requestAnimationFrame` で線形補間して 60 fps の滑らかな
 * 再生位置を返す。
 *
 * 補間中は `snapshot.position_sec + (now - snapshot_arrival) * playback_speed`
 * を使う。pause / stop の場合は固定値。
 */
export function useInterpolatedPosition(snapshot: DeckSnapshot): number {
  const baseRef = useRef({
    pos: snapshot.position_sec,
    arrivedAt: performance.now(),
    speed: snapshot.playback_speed,
    playing: snapshot.state === "play",
  });

  // snapshot が更新されたら base を差し替える
  useEffect(() => {
    baseRef.current = {
      pos: snapshot.position_sec,
      arrivedAt: performance.now(),
      speed: snapshot.playback_speed,
      playing: snapshot.state === "play",
    };
  }, [snapshot.position_sec, snapshot.playback_speed, snapshot.state]);

  const [livePos, setLivePos] = useState(snapshot.position_sec);

  useEffect(() => {
    let raf = 0;
    const tick = () => {
      const { pos, arrivedAt, speed, playing } = baseRef.current;
      const next = playing ? pos + ((performance.now() - arrivedAt) / 1000) * speed : pos;
      setLivePos(next);
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);

  return livePos;
}
