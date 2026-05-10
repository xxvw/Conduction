import { useState } from "react";

import type { CueDto, CueTypeId, MixRoleId } from "@/lib/ipc";

interface CuePadProps {
  cues: CueDto[];
  onAdd: (args: {
    cueType: CueTypeId;
    phraseLength: number;
    mixRoles: MixRoleId[];
  }) => void;
  onDelete: (cueId: string) => void;
  /** 操作対象のトラックがロード済みか (= 追加可能か)。 */
  enabled: boolean;
}

const CUE_TYPES: { id: CueTypeId; label: string }[] = [
  { id: "drop", label: "DROP" },
  { id: "intro_start", label: "INTRO IN" },
  { id: "intro_end", label: "INTRO OUT" },
  { id: "breakdown", label: "BREAKDOWN" },
  { id: "outro", label: "OUTRO" },
  { id: "custom_hot_cue", label: "CUSTOM" },
  { id: "hot_cue", label: "HOT" },
];

const PHRASE_OPTIONS = [8, 16, 32, 64];

export function CuePad({ cues, onAdd, onDelete, enabled }: CuePadProps) {
  const [cueType, setCueType] = useState<CueTypeId>("drop");
  const [phraseLength, setPhraseLength] = useState<number>(32);
  const [entry, setEntry] = useState<boolean>(true);
  const [exit, setExit] = useState<boolean>(false);

  const handleAdd = () => {
    if (!enabled) return;
    const roles: MixRoleId[] = [];
    if (entry) roles.push("entry");
    if (exit) roles.push("exit");
    onAdd({ cueType, phraseLength, mixRoles: roles });
  };

  return (
    <div className="cue-pad">
      <div className="cue-pad-row">
        <select
          className="cue-select"
          value={cueType}
          onChange={(e) => setCueType(e.target.value as CueTypeId)}
          disabled={!enabled}
        >
          {CUE_TYPES.map((t) => (
            <option key={t.id} value={t.id}>
              {t.label}
            </option>
          ))}
        </select>
        <select
          className="cue-select"
          value={phraseLength}
          onChange={(e) => setPhraseLength(Number(e.target.value))}
          disabled={!enabled}
          aria-label="phrase length (beats)"
          title="phrase length (beats)"
        >
          {PHRASE_OPTIONS.map((n) => (
            <option key={n} value={n}>
              {n}b
            </option>
          ))}
        </select>
        <label className="cue-role" title="usable as Entry">
          <input
            type="checkbox"
            checked={entry}
            onChange={(e) => setEntry(e.target.checked)}
            disabled={!enabled}
          />
          IN
        </label>
        <label className="cue-role" title="usable as Exit">
          <input
            type="checkbox"
            checked={exit}
            onChange={(e) => setExit(e.target.checked)}
            disabled={!enabled}
          />
          OUT
        </label>
        <button
          type="button"
          className="cue-add-btn"
          onClick={handleAdd}
          disabled={!enabled}
          title="add cue at current position"
        >
          + Cue
        </button>
      </div>
      {cues.length > 0 && (
        <ul className="cue-list">
          {cues.map((c) => (
            <CueChip key={c.id} cue={c} onDelete={() => onDelete(c.id)} />
          ))}
        </ul>
      )}
    </div>
  );
}

function CueChip({ cue, onDelete }: { cue: CueDto; onDelete: () => void }) {
  const t = CUE_TYPES.find((x) => x.id === cue.cue_type)?.label ?? cue.cue_type;
  const roles = cue.mixable_as.map((r) => (r === "entry" ? "IN" : "OUT")).join("/");
  return (
    <li className="cue-chip" data-type={cue.cue_type}>
      <span className="cue-chip-type">{t}</span>
      <span className="cue-chip-pos">@{cue.position_beats.toFixed(0)}b</span>
      {roles && <span className="cue-chip-role">{roles}</span>}
      <button
        type="button"
        className="cue-chip-del"
        onClick={onDelete}
        aria-label="delete cue"
        title="delete"
      >
        ×
      </button>
    </li>
  );
}
