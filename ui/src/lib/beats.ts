import type { BeatDto } from "@/types/beat";

/**
 * 与えられた秒位置を、`beats` の中で最も近い拍位置にスナップして返す。
 * `beats` が空（= BPM 未推定 / ビートグリッド未生成）の場合は元の値をそのまま返す。
 *
 * `beats` は position_sec 昇順を前提（DB から `ORDER BY position_sec ASC` で
 * 取得しているので満たされる）。
 */
export function snapToNearestBeat(targetSec: number, beats: BeatDto[]): number {
  if (!beats || beats.length === 0) return targetSec;
  // targetSec 以上の最初の拍を二分探索で見つける
  let lo = 0;
  let hi = beats.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (beats[mid]!.position_sec < targetSec) lo = mid + 1;
    else hi = mid;
  }
  const next = lo < beats.length ? beats[lo] : undefined;
  const prev = lo > 0 ? beats[lo - 1] : undefined;
  if (next && prev) {
    const dn = Math.abs(next.position_sec - targetSec);
    const dp = Math.abs(prev.position_sec - targetSec);
    return dp <= dn ? prev.position_sec : next.position_sec;
  }
  return prev?.position_sec ?? next?.position_sec ?? targetSec;
}

/**
 * 与えられた秒位置に最も近い拍の **インデックス (= 拍数)** を返す。
 * 0 origin。Cue の `position_beats` を計算するのに使う。
 * `beats` が空なら BPM 仮定 120 で sec * 2 で近似する。
 */
export function secondsToBeatIndex(sec: number, beats: BeatDto[]): number {
  if (!beats || beats.length === 0) {
    return Math.max(0, Math.round(sec * 2)); // 120 BPM ≒ 2 beats/sec
  }
  let lo = 0;
  let hi = beats.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (beats[mid]!.position_sec < sec) lo = mid + 1;
    else hi = mid;
  }
  const next = lo < beats.length ? lo : beats.length - 1;
  const prev = Math.max(0, lo - 1);
  const dn = Math.abs(beats[next]!.position_sec - sec);
  const dp = Math.abs(beats[prev]!.position_sec - sec);
  return dp <= dn ? prev : next;
}
