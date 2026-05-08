import { useEffect, useRef } from "react";

import { displayKey, type ShortcutBinding } from "@/lib/keybindings";

interface KeyConfigModalProps {
  open: boolean;
  onClose: () => void;
  bindings: ShortcutBinding[];
  activeDeck: "A" | "B";
  zoomWindowSec: number;
}

export function KeyConfigModal({
  open,
  onClose,
  bindings,
  activeDeck,
  zoomWindowSec,
}: KeyConfigModalProps) {
  const closeBtnRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    closeBtnRef.current?.focus();
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;
  return (
    <div
      className="keyconfig-modal-backdrop"
      role="presentation"
      onClick={onClose}
    >
      <div
        className="keyconfig-modal"
        role="dialog"
        aria-modal="true"
        aria-label="keyboard shortcuts"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="keyconfig-modal-head">
          <h3 className="keyconfig-modal-title">Keyboard Shortcuts</h3>
          <button
            ref={closeBtnRef}
            type="button"
            className="keyconfig-modal-close"
            onClick={onClose}
            aria-label="close"
          >
            ×
          </button>
        </div>

        <div className="keyconfig-modal-status">
          <span className="keyconfig-active">
            <span className="keyconfig-active-label">FOCUS</span>
            <span className="keyconfig-active-deck" data-id={activeDeck}>
              DECK {activeDeck}
            </span>
          </span>
          <span className="keyconfig-active">
            <span className="keyconfig-active-label">ZOOM</span>
            <span className="keyconfig-zoom-value">±{zoomWindowSec}s</span>
          </span>
        </div>

        <div className="keyconfig-modal-grid">
          {bindings.map((b) => (
            <div key={b.key} className="keyconfig-item">
              <kbd className="keyconfig-key">{displayKey(b.key)}</kbd>
              <span className="keyconfig-label">{b.label}</span>
            </div>
          ))}
        </div>

        <div className="keyconfig-modal-foot">
          <span className="keyconfig-item keyconfig-hint">
            <kbd className="keyconfig-key">Shift</kbd>
            <span className="keyconfig-label">+ seek key = fine (no snap)</span>
          </span>
          <span className="keyconfig-item keyconfig-hint">
            <kbd className="keyconfig-key">Esc</kbd>
            <span className="keyconfig-label">close</span>
          </span>
        </div>
      </div>
    </div>
  );
}
