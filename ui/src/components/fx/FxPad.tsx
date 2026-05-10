import { ipc } from "@/lib/ipc";
import type {
  AutomationModeKind,
  DeckId,
  DeckSnapshot,
  MixerSnapshot,
} from "@/types/mixer";

interface FxPadProps {
  deckId: DeckId;
  snapshot: DeckSnapshot;
  mixerStatus: MixerSnapshot | null;
  focusedTarget: string;
  onFocus: (key: string) => void;
}

function lookupMode(
  status: MixerSnapshot | null,
  key: string,
): AutomationModeKind {
  return (
    status?.template?.automation_modes.find((m) => m.target_key === key)?.mode ??
    "idle"
  );
}

const KILL_DB = -40; // Kill 押下時に適用するゲイン
const MAX_DB = 6;
const MIN_DB = -26;

export function FxPad({
  deckId,
  snapshot,
  mixerStatus,
  focusedTarget,
  onFocus,
}: FxPadProps) {
  const isKill = (db: number) => db <= KILL_DB + 0.5;

  const handleEq = (band: "low" | "mid" | "high", db: number) => {
    void ipc.setEq(deckId, band, db);
  };

  const eqLowKey = `deck_eq_low.${deckId}`;
  const eqMidKey = `deck_eq_mid.${deckId}`;
  const eqHighKey = `deck_eq_high.${deckId}`;
  const filterKey = `deck_filter.${deckId}`;

  return (
    <div className="fx-pad" data-deck={deckId} aria-label={`Effects for Deck ${deckId}`}>
      <div className="fx-section">
        <div className="fx-section-title">EQ</div>
        <EqRow
          label="HI"
          value={snapshot.eq_high_db}
          onChange={(v) => handleEq("high", v)}
          isKill={isKill(snapshot.eq_high_db)}
          onKill={() => handleEq("high", isKill(snapshot.eq_high_db) ? 0 : KILL_DB)}
          targetKey={eqHighKey}
          mode={lookupMode(mixerStatus, eqHighKey)}
          focused={focusedTarget === eqHighKey}
          onFocus={() => onFocus(eqHighKey)}
        />
        <EqRow
          label="MID"
          value={snapshot.eq_mid_db}
          onChange={(v) => handleEq("mid", v)}
          isKill={isKill(snapshot.eq_mid_db)}
          onKill={() => handleEq("mid", isKill(snapshot.eq_mid_db) ? 0 : KILL_DB)}
          targetKey={eqMidKey}
          mode={lookupMode(mixerStatus, eqMidKey)}
          focused={focusedTarget === eqMidKey}
          onFocus={() => onFocus(eqMidKey)}
        />
        <EqRow
          label="LOW"
          value={snapshot.eq_low_db}
          onChange={(v) => handleEq("low", v)}
          isKill={isKill(snapshot.eq_low_db)}
          onKill={() => handleEq("low", isKill(snapshot.eq_low_db) ? 0 : KILL_DB)}
          targetKey={eqLowKey}
          mode={lookupMode(mixerStatus, eqLowKey)}
          focused={focusedTarget === eqLowKey}
          onFocus={() => onFocus(eqLowKey)}
        />
      </div>

      <div
        className="fx-section param-control"
        data-target={filterKey}
        data-mode={lookupMode(mixerStatus, filterKey)}
        data-focused={focusedTarget === filterKey}
        onClick={() => onFocus(filterKey)}
      >
        <div className="fx-section-title">FILTER</div>
        <div className="fx-knob-row">
          <input
            type="range"
            min={-1}
            max={1}
            step={0.01}
            value={snapshot.filter}
            onChange={(e) => void ipc.setFilter(deckId, parseFloat(e.target.value))}
          />
          <button
            className="fx-mini-btn"
            title="Reset filter"
            onClick={(e) => {
              e.stopPropagation();
              void ipc.setFilter(deckId, 0);
            }}
          >
            ⟲
          </button>
        </div>
        <div className="fx-readout">
          <span>LPF</span>
          <span className="fx-readout-value">{filterLabel(snapshot.filter)}</span>
          <span>HPF</span>
        </div>
      </div>

      <div className="fx-section">
        <div className="fx-section-title">ECHO</div>
        <FxEffectRow
          label="wet"
          value={snapshot.echo_wet}
          min={0}
          max={1}
          step={0.01}
          format={(v) => `${(v * 100).toFixed(0)}%`}
          onChange={(v) =>
            void ipc.setEcho(deckId, v, snapshot.echo_time_ms, snapshot.echo_feedback)
          }
        />
        <FxEffectRow
          label="time"
          value={snapshot.echo_time_ms}
          min={50}
          max={1500}
          step={5}
          format={(v) => `${Math.round(v)} ms`}
          onChange={(v) =>
            void ipc.setEcho(deckId, snapshot.echo_wet, v, snapshot.echo_feedback)
          }
        />
        <FxEffectRow
          label="fb"
          value={snapshot.echo_feedback}
          min={0}
          max={0.92}
          step={0.01}
          format={(v) => `${(v * 100).toFixed(0)}%`}
          onChange={(v) =>
            void ipc.setEcho(deckId, snapshot.echo_wet, snapshot.echo_time_ms, v)
          }
        />
      </div>

      <div className="fx-section">
        <div className="fx-section-title">REVERB</div>
        <FxEffectRow
          label="wet"
          value={snapshot.reverb_wet}
          min={0}
          max={1}
          step={0.01}
          format={(v) => `${(v * 100).toFixed(0)}%`}
          onChange={(v) => void ipc.setReverb(deckId, v, snapshot.reverb_room)}
        />
        <FxEffectRow
          label="room"
          value={snapshot.reverb_room}
          min={0}
          max={1}
          step={0.01}
          format={(v) => `${(v * 100).toFixed(0)}%`}
          onChange={(v) => void ipc.setReverb(deckId, snapshot.reverb_wet, v)}
        />
      </div>
    </div>
  );
}

function EqRow({
  label,
  value,
  onChange,
  isKill,
  onKill,
  targetKey,
  mode,
  focused,
  onFocus,
}: {
  label: string;
  value: number;
  onChange: (db: number) => void;
  isKill: boolean;
  onKill: () => void;
  targetKey: string;
  mode: AutomationModeKind;
  focused: boolean;
  onFocus: () => void;
}) {
  return (
    <div
      className="fx-row eq-row param-control"
      data-target={targetKey}
      data-mode={mode}
      data-focused={focused}
      onClick={onFocus}
    >
      <span className="fx-row-label">{label}</span>
      <input
        type="range"
        min={MIN_DB}
        max={MAX_DB}
        step={0.5}
        value={value}
        onChange={(e) => onChange(parseFloat(e.target.value))}
      />
      <span className="fx-row-value">{formatDb(value)}</span>
      <button
        className="eq-kill"
        data-active={isKill || undefined}
        title="Kill (silence this band)"
        onClick={(e) => {
          e.stopPropagation();
          onKill();
        }}
      >
        K
      </button>
    </div>
  );
}

function FxEffectRow({
  label,
  value,
  min,
  max,
  step,
  format,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  format: (v: number) => string;
  onChange: (v: number) => void;
}) {
  return (
    <div className="fx-row">
      <span className="fx-row-label">{label}</span>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(parseFloat(e.target.value))}
      />
      <span className="fx-row-value">{format(value)}</span>
    </div>
  );
}

function formatDb(db: number): string {
  if (db <= -39.5) return "−∞";
  return `${db >= 0 ? "+" : ""}${db.toFixed(1)}`;
}

function filterLabel(v: number): string {
  if (Math.abs(v) < 0.02) return "—";
  if (v < 0) return `LPF ${Math.round(-v * 100)}%`;
  return `HPF ${Math.round(v * 100)}%`;
}
