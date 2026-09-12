import { useEffect, useRef, type ButtonHTMLAttributes } from "react";

export function formatTime(seconds: number): string {
  const value = Number.isFinite(seconds) ? Math.max(0, seconds) : 0;
  return `${Math.floor(value / 60).toString().padStart(2, "0")}:${Math.floor(value % 60).toString().padStart(2, "0")}`;
}

export function Range({ label, value, min, max, step = 0.01, suffix = "", disabled, onChange, vertical }: {
  label: string; value: number; min: number; max: number; step?: number;
  suffix?: string; disabled?: boolean; onChange: (value: number) => void; vertical?: boolean;
}) {
  const numeric = Number.isFinite(value) ? value : min;
  return <label className="ctl-range" data-vertical={vertical || undefined}>
    <span className="ctl-range-label">{label}</span>
    <input type="range" aria-label={label} min={min} max={max} step={step}
      value={numeric} disabled={disabled} onChange={(event) => onChange(Number(event.target.value))} />
    <output>{numeric.toFixed(step < 1 ? (step < 0.1 ? 2 : 1) : 0)}{suffix}</output>
  </label>;
}

/** Calibrated from the audio callback's peak amplitude; never inferred from gain. */
export function Meter({ levels, label, compact }: { levels: readonly number[]; label: string; compact?: boolean }) {
  return <div className="ctl-meter" data-compact={compact || undefined} role="img"
    aria-label={`${label}: ${levels.map((level) => level > 0 ? `${(20 * Math.log10(level)).toFixed(1)} dBFS` : "silent").join(", ")}`}>
    {levels.map((level, index) => {
      const db = level > 0 ? 20 * Math.log10(level) : -60;
      const amount = Math.max(0, Math.min(1, (db + 60) / 60));
      return <span className="ctl-meter-track" key={index} data-clipped={level >= 1 || undefined}>
        <span className="ctl-meter-fill" style={{ height: `${amount * 100}%` }} />
      </span>;
    })}
  </div>;
}

/** A gate releases on cancellation, keyboard release, window blur and unmount. */
export function HoldButton({ onHold, onError, children, ...props }: Omit<ButtonHTMLAttributes<HTMLButtonElement>, "onError"> & {
  onHold: (pressed: boolean) => Promise<void>; onError: (message: string) => void;
}) {
  const held = useRef(false);
  const callback = useRef(onHold);
  const error = useRef(onError);
  const queue = useRef(Promise.resolve());
  callback.current = onHold;
  error.current = onError;
  const change = (pressed: boolean) => {
    if (pressed === held.current) return;
    held.current = pressed;
    queue.current = queue.current.catch(() => {}).then(() => callback.current(pressed)).catch((reason: unknown) => error.current(String(reason)));
  };
  useEffect(() => {
    const release = () => {
      if (!held.current) return;
      held.current = false;
      queue.current = queue.current.catch(() => {}).then(() => callback.current(false)).catch((reason: unknown) => error.current(String(reason)));
    };
    window.addEventListener("blur", release);
    return () => { window.removeEventListener("blur", release); release(); };
  }, []);
  return <button {...props} type="button"
    onPointerDown={(event) => { if (event.button !== 0) return; event.preventDefault(); event.currentTarget.focus(); event.currentTarget.setPointerCapture(event.pointerId); change(true); }}
    onPointerUp={() => change(false)} onPointerCancel={() => change(false)} onLostPointerCapture={() => change(false)} onBlur={() => change(false)}
    onKeyDown={(event) => { if (event.key === " " || event.key === "Enter") { event.preventDefault(); event.stopPropagation(); if (!event.repeat) change(true); } }}
    onKeyUp={(event) => { if (event.key === " " || event.key === "Enter") { event.preventDefault(); event.stopPropagation(); change(false); } }}>
    {children}
  </button>;
}
