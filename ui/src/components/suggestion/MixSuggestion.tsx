import type { MatchCandidate } from "@/lib/ipc";
import type { DeckId } from "@/types/mixer";

interface MixSuggestionProps {
  open: boolean;
  /** どのデッキに「対して」候補を提案しているか (= 反対デッキ)。 */
  targetDeck: DeckId;
  candidates: MatchCandidate[];
  onPick: (c: MatchCandidate) => void;
  onDismiss: () => void;
}

export function MixSuggestion({
  open,
  targetDeck,
  candidates,
  onPick,
  onDismiss,
}: MixSuggestionProps) {
  if (!open) return null;
  return (
    <aside className="mix-suggestion glass-strong" aria-label="mix suggestions">
      <header className="mix-suggestion-head">
        <span className="mix-suggestion-title">
          Next Cue · 繋ぎ候補
        </span>
        <span className="mix-suggestion-target" data-id={targetDeck}>
          → DECK {targetDeck}
        </span>
        <button
          type="button"
          className="mix-suggestion-close"
          onClick={onDismiss}
          aria-label="dismiss"
          title="Esc"
        >
          ×
        </button>
      </header>
      {candidates.length === 0 ? (
        <p className="mix-suggestion-empty">No compatible cues yet.</p>
      ) : (
        <ul className="mix-suggestion-list">
          {candidates.map((c, idx) => (
            <li key={c.cue.id} className="mix-suggestion-row">
              <button
                type="button"
                className="mix-suggestion-pick"
                onClick={() => onPick(c)}
                title={
                  idx === 0
                    ? "Enter — pick top candidate"
                    : `Pick (BPM ${c.bpm_score.toFixed(2)} / Key ${c.key_score.toFixed(2)} / E ${c.energy_score.toFixed(2)})`
                }
              >
                <span
                  className="mix-suggestion-pct"
                  data-strength={
                    c.overall_score >= 0.85
                      ? "hi"
                      : c.overall_score >= 0.6
                      ? "mid"
                      : "lo"
                  }
                >
                  {Math.round(c.overall_score * 100)}%
                </span>
                <span className="mix-suggestion-meta">
                  <span className="mix-suggestion-track-title">
                    {c.track.title || c.track.path.split("/").pop() || "(untitled)"}
                  </span>
                  <span className="mix-suggestion-track-sub">
                    {c.track.artist || "—"} · {c.track.bpm.toFixed(1)} BPM · {c.track.key}
                  </span>
                </span>
                <span
                  className="mix-suggestion-cue-type"
                  data-type={c.cue.cue_type}
                >
                  {cueTypeLabel(c.cue.cue_type)}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </aside>
  );
}

function cueTypeLabel(t: string): string {
  switch (t) {
    case "drop":
      return "DROP";
    case "intro_start":
      return "IN";
    case "intro_end":
      return "INTRO/END";
    case "breakdown":
      return "BREAK";
    case "outro":
      return "OUTRO";
    case "custom_hot_cue":
      return "CUSTOM";
    case "hot_cue":
      return "HOT";
    default:
      return t.toUpperCase();
  }
}
