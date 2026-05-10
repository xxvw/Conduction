/**
 * Camelot 表記 (例: "8A", "11B") を MIDI ピッチクラス (0=C..11=B) に変換し、
 * 2 つの key 間で「最も近い」半音差を返すヘルパー群。KEY SYNC 用。
 */

const MAJOR_PC: ReadonlyArray<number> = [
  // index = camelot 番号 - 1。位置 1 = B major (PC 11) から始まる。
  11, 6, 1, 8, 3, 10, 5, 0, 7, 2, 9, 4,
];
const MINOR_PC: ReadonlyArray<number> = [
  // 位置 1 = G# minor (PC 8) から
  8, 3, 10, 5, 0, 7, 2, 9, 4, 11, 6, 1,
];

export function camelotToPitchClass(camelot: string | null | undefined): number | null {
  if (!camelot) return null;
  const m = camelot.trim().match(/^(\d+)([AaBb])$/);
  if (!m) return null;
  const num = parseInt(m[1]!, 10);
  if (num < 1 || num > 12) return null;
  const isMajor = m[2]!.toUpperCase() === "B";
  const table = isMajor ? MAJOR_PC : MINOR_PC;
  return table[num - 1] ?? null;
}

/**
 * own → target に転調するときの最短半音差を返す (-6..=6)。
 * 例: own=8A (Am, PC 9), target=9A (Em, PC 4) → -5
 */
export function shortestSemitoneDiff(
  own: string | null | undefined,
  target: string | null | undefined,
): number | null {
  const a = camelotToPitchClass(own);
  const b = camelotToPitchClass(target);
  if (a == null || b == null) return null;
  let d = ((b - a) % 12 + 12) % 12;
  if (d > 6) d -= 12;
  return d;
}
