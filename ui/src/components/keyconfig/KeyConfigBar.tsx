import { displayKey, type ShortcutBinding } from "@/lib/keybindings";

interface KeyConfigBarProps {
  bindings: ShortcutBinding[];
  /** 現在のアクティブデッキ（A/B）— ハイライト用 */
  activeDeck: "A" | "B";
  /** ズーム窓の半幅（秒） — 状態表示用 */
  zoomWindowSec: number;
}

/**
 * 画面下部に薄く配置するキーボードショートカット早見表。
 * Phase 3b-10 で設定画面から編集できるようにする。
 */
export function KeyConfigBar({ bindings, activeDeck, zoomWindowSec }: KeyConfigBarProps) {
  return (
    <div className="keyconfig-bar" role="region" aria-label="keyboard shortcuts">
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
      <span className="keyconfig-divider" />
      {bindings.map((b) => (
        <span key={b.key} className="keyconfig-item">
          <kbd className="keyconfig-key">{displayKey(b.key)}</kbd>
          <span className="keyconfig-label">{b.label}</span>
        </span>
      ))}
    </div>
  );
}
