import { useState } from "react";

import { ipc } from "@/lib/ipc";
import type { DeckId, DeckSnapshot, MixerSnapshot } from "@/types/mixer";
import type { ControllerAction } from "@/types/performance";
import { Meter, Range } from "./ControllerControls";

interface ControllerMixerProps {
  status: MixerSnapshot;
  external: boolean;
  dispatch: (action: ControllerAction) => Promise<void>;
  onError: (message: string) => void;
}

/** Keep the center detent at 0 dB, matching the shared MIDI dispatcher. */
function eqValue(db: number): number {
  return Math.max(0, Math.min(1, db <= 0 ? (db + 24) / 48 : 0.5 + db / 24));
}

function usesInternalMixer(targetKey: string): boolean {
  return /^(crossfader|master_volume|deck_volume|deck_eq_(low|mid|high)|deck_filter)(\.|$)/.test(targetKey);
}

export function ControllerMixer({ status, external, dispatch, onError }: ControllerMixerProps) {
  const [automationBusy, setAutomationBusy] = useState(false);
  const send = (action: ControllerAction) => {
    void dispatch(action).catch((error: unknown) => onError(String(error)));
  };
  const template = status.template;
  const modes = template?.automation_modes.filter((entry) => !external || !usesInternalMixer(entry.target_key)) ?? [];
  const overrideTargets = modes.filter((entry) => entry.mode === "automated" || entry.mode === "resuming");
  const resumeTargets = modes.filter((entry) => entry.mode === "overridden" || entry.mode === "committed");

  const changeAutomation = async (operation: "override" | "resume") => {
    if (automationBusy) return;
    const targets = operation === "override" ? overrideTargets : resumeTargets;
    if (!targets.length) return;
    setAutomationBusy(true);
    try {
      const results = await Promise.allSettled(targets.map(({ target_key }) =>
        operation === "override" ? ipc.overrideParam(target_key) : ipc.resumeParam(target_key, 4),
      ));
      const failures = results.flatMap((result, index) => result.status === "rejected"
        ? [`${targets[index]?.target_key ?? "Automation"}: ${String(result.reason)}`]
        : []);
      if (failures.length) onError(failures.join("\n"));
    } catch (error) {
      onError(String(error));
    } finally {
      setAutomationBusy(false);
    }
  };

  return <section className="ctl-mixer" aria-label="DJ mixer" data-external={external || undefined}>
    <header className="ctl-section-heading">
      <h2>MIXER</h2>
      <span className="ctl-badge">{external ? "EXTERNAL" : "INTERNAL"}</span>
    </header>
    {external && <p className="ctl-mixer-bypass" role="note">
      EQ・Filter・フェーダーはDJMで操作。デッキFXは有効です。
    </p>}

    <div className="ctl-mixer-channels">
      <Channel deck="A" snapshot={status.deck_a} levels={status.audio.peak_decks[0]}
        external={external} send={send} />
      <Channel deck="B" snapshot={status.deck_b} levels={status.audio.peak_decks[1]}
        external={external} send={send} />
    </div>

    <div className="ctl-crossfader">
      <Range label="CROSSFADER A ↔ B" value={status.crossfader} min={-1} max={1} step={0.01}
        disabled={external} onChange={(value) => send({ action: "crossfader", value: (value + 1) / 2 })} />
    </div>

    <div className="ctl-master-strip">
      <Range label="MASTER" value={status.master_volume * 100} min={0} max={200} step={1} suffix="%"
        disabled={external} onChange={(value) => send({ action: "master", value: value / 200 })} />
      <Meter label="Main output" levels={status.audio.peak_main} compact />
    </div>

    <div className="ctl-headphones">
      <div className="ctl-section-heading"><h3>HEADPHONES</h3><span>CUE / MASTER</span></div>
      <Range label="CUE ↔ MASTER" value={status.headphone_mix * 100} min={0} max={100} step={1} suffix="%"
        onChange={(value) => send({ action: "headphone_mix", value: value / 100 })} />
      <div className="ctl-master-strip">
        <Range label="HEADPHONE LEVEL" value={status.headphone_volume * 100} min={0} max={200} step={1} suffix="%"
          onChange={(value) => send({ action: "headphone_volume", value: value / 200 })} />
        <Meter label="Headphone output" levels={status.audio.peak_cue} compact />
      </div>
    </div>

    <div className="ctl-automation" aria-label="Template automation" data-active={Boolean(template) || undefined}>
      <div className="ctl-section-heading">
        <h3>AUTOMATION</h3>
        <span>{template ? `${Math.max(0, template.beats_remaining).toFixed(1)} beats` : "MANUAL"}</span>
      </div>
      <p className="ctl-automation-name" title={template?.name}>{template?.name ?? "テンプレート停止中"}</p>
      <progress aria-label="Template progress" max={1} value={Math.max(0, Math.min(1, template?.progress ?? 0))} />
      <div className="ctl-button-row">
        <button type="button" className="ctl-button" disabled={automationBusy || !overrideTargets.length}
          title="自動化中のパラメーターを手動操作へ切り替える"
          onClick={() => void changeAutomation("override")}>Override</button>
        <button type="button" className="ctl-button" disabled={automationBusy || !resumeTargets.length}
          title="4拍かけてテンプレートの値へ戻す"
          onClick={() => void changeAutomation("resume")}>Resume · 4 beats</button>
      </div>
      {!!template?.override_count && <span className="ctl-muted">{template.override_count} overrides</span>}
    </div>
  </section>;
}

function Channel({ deck, snapshot, levels, external, send }: {
  deck: DeckId;
  snapshot: DeckSnapshot;
  levels: readonly number[];
  external: boolean;
  send: (action: ControllerAction) => void;
}) {
  const [effect, setEffect] = useState<"echo_wet" | "reverb_wet">("echo_wet");
  return <div className="ctl-mixer-channel" data-deck={deck} aria-label={`Deck ${deck} mixer channel`}>
    <h3 className="ctl-channel-label">{deck}</h3>
    <div className="ctl-channel-eq">
      <Range label={`${deck} HI`} value={snapshot.eq_high_db} min={-24} max={12} step={0.5} suffix=" dB"
        disabled={external} onChange={(value) => send({ action: "eq", deck, band: "high", value: eqValue(value) })} />
      <Range label={`${deck} MID`} value={snapshot.eq_mid_db} min={-24} max={12} step={0.5} suffix=" dB"
        disabled={external} onChange={(value) => send({ action: "eq", deck, band: "mid", value: eqValue(value) })} />
      <Range label={`${deck} LOW`} value={snapshot.eq_low_db} min={-24} max={12} step={0.5} suffix=" dB"
        disabled={external} onChange={(value) => send({ action: "eq", deck, band: "low", value: eqValue(value) })} />
      <Range label={`${deck} LPF ↔ HPF`} value={snapshot.filter} min={-1} max={1} step={0.01}
        disabled={external} onChange={(value) => send({ action: "filter", deck, value: (value + 1) / 2 })} />
    </div>

    <div className="ctl-channel-fx">
      <label className="ctl-fx-selector">
        <span>{deck} FX</span>
        <select aria-label={`Deck ${deck} effect`} value={effect}
          onChange={(event) => setEffect(event.target.value === "reverb_wet" ? "reverb_wet" : "echo_wet")}>
          <option value="echo_wet">ECHO</option>
          <option value="reverb_wet">REVERB</option>
        </select>
      </label>
      <Range label={`${deck} FX WET`} value={snapshot[effect] * 100} min={0} max={100} step={1} suffix="%"
        onChange={(value) => send({ action: "fx", deck, parameter: effect, value: value / 100 })} />
    </div>

    <button type="button" className="ctl-button ctl-headphone-cue" aria-pressed={snapshot.cue_send > 0}
      aria-label={`Deck ${deck} headphone cue`} title="ヘッドホンへの送信（Transport Cueとは別操作）"
      onClick={() => send({ action: "headphone_cue", deck })}>HEADPHONE CUE</button>
    <div className="ctl-channel-fader">
      <Range label={`${deck} LEVEL`} value={snapshot.channel_volume * 100} min={0} max={200} step={1} suffix="%" vertical
        disabled={external} onChange={(value) => send({ action: "fader", deck, value: value / 200 })} />
      <Meter label={`Deck ${deck} audio`} levels={levels} />
    </div>
  </div>;
}
