import type { DeckId } from "@/types/mixer";
import type { HotCueDto } from "@/types/hotcue";

interface HotCuePadProps {
  deckId: DeckId;
  cues: HotCueDto[];
  currentPositionSec: number;
  onJump: (slot: number) => void;
  onSet: (slot: number, positionSec: number) => void;
  onDelete: (slot: number) => void;
}

const SLOTS = [1, 2, 3, 4, 5, 6, 7, 8] as const;

export function HotCuePad({
  deckId,
  cues,
  currentPositionSec,
  onJump,
  onSet,
  onDelete,
}: HotCuePadProps) {
  const cueBySlot = new Map(cues.map((c) => [c.slot, c]));
  return (
    <div className="hotcue-pad" data-deck={deckId}>
      {SLOTS.map((slot) => {
        const cue = cueBySlot.get(slot) ?? null;
        return (
          <button
            key={slot}
            className="hotcue-pad-btn"
            data-deck={deckId}
            data-empty={cue == null}
            title={
              cue
                ? `Hot Cue ${slot} @ ${formatSec(cue.position_sec)}`
                : `Hot Cue ${slot} (empty)`
            }
            onClick={(e) => {
              if (e.altKey) onDelete(slot);
              else if (e.shiftKey) onSet(slot, currentPositionSec);
              else if (cue) onJump(slot);
              else onSet(slot, currentPositionSec);
            }}
          >
            <span className="hotcue-slot-num">{slot}</span>
            <span className="hotcue-slot-pos">
              {cue ? formatSec(cue.position_sec) : "—"}
            </span>
          </button>
        );
      })}
    </div>
  );
}

function formatSec(sec: number): string {
  if (!Number.isFinite(sec) || sec < 0) sec = 0;
  const m = Math.floor(sec / 60);
  const s = Math.floor(sec % 60);
  return `${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
}
