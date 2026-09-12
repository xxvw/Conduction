import { useCallback, useEffect, useRef, useState } from "react";
import type { FormEvent, KeyboardEvent, ReactNode } from "react";

import { performanceIpc } from "@/lib/performance";
import type { DeckId, MixerSnapshot } from "@/types/mixer";
import type {
  AudioOutputConfig,
  AudioOutputDescriptor,
  LinkConfig,
  LinkInterface,
  MidiBinding,
  MidiConfig,
  MidiDevices,
  MidiProfile,
  PerformanceStatus,
} from "@/types/performance";

type ConnectionTab = "audio" | "link" | "midi";

interface ControllerConnectionsProps {
  performance: PerformanceStatus;
  mixer: MixerSnapshot;
  onClose: () => void;
  onError: (message: string) => void;
  onRefresh: () => void;
  initialTab?: ConnectionTab;
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return <label className="ctl-field"><span>{label}</span>{children}</label>;
}

function stereoPairs(channels: number): number[] {
  return Array.from({ length: Math.floor(Math.max(0, channels) / 2) }, (_, index) => index * 2);
}

function bindingCount(config: MidiConfig, profiles: MidiProfile[]): number | string {
  return config.bindings.length > 0
    ? config.bindings.length
    : profiles.find((profile) => profile.id === config.profile)?.bindings.length ?? "—";
}

function PairSelect({ label, value, channels, optional = false, onChange }: {
  label: string;
  value: number | null;
  channels: number;
  optional?: boolean;
  onChange: (value: number | null) => void;
}) {
  const pairs = stereoPairs(channels);
  return (
    <Field label={label}>
      <select value={value ?? "none"} onChange={(event) => onChange(event.target.value === "none" ? null : Number(event.target.value))}>
        {optional && <option value="none">出力なし</option>}
        {value !== null && !pairs.includes(value) && <option value={value} disabled>{value + 1} / {value + 2} — 利用不可</option>}
        {pairs.map((pair) => <option key={pair} value={pair}>{pair + 1} / {pair + 2}</option>)}
      </select>
    </Field>
  );
}

export function ControllerConnections({
  performance, mixer, onClose, onError, onRefresh, initialTab = "audio",
}: ControllerConnectionsProps) {
  const [tab, setTab] = useState<ConnectionTab>(initialTab);
  // Connection settings remain local drafts until Apply; status polls only
  // update the live status below the forms.
  const [audio, setAudio] = useState<AudioOutputConfig>(() => ({ ...performance.audio_config }));
  const [link, setLink] = useState<LinkConfig>(() => ({ ...performance.link_config, mac_address: [...performance.link_config.mac_address] }));
  const [midi, setMidi] = useState<MidiConfig>({ input_port: "", output_port: null, profile: "", deck: null, bindings: [] });
  const [outputs, setOutputs] = useState<AudioOutputDescriptor[]>([]);
  const [interfaces, setInterfaces] = useState<LinkInterface[]>([]);
  const [devices, setDevices] = useState<MidiDevices>({ inputs: [], outputs: [] });
  const [profiles, setProfiles] = useState<MidiProfile[]>([]);
  const [discoveryErrors, setDiscoveryErrors] = useState<string[]>([]);
  const [discovering, setDiscovering] = useState(false);
  const [pending, setPending] = useState<string | null>(null);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [customBindings, setCustomBindings] = useState(false);
  const [bindingText, setBindingText] = useState("[]");
  const dialog = useRef<HTMLDivElement>(null);
  const closeButton = useRef<HTMLButtonElement>(null);
  const alive = useRef(false);
  const discoveryRequest = useRef(0);
  const busy = useRef(false);
  const callbacks = useRef({ onError, onRefresh });
  callbacks.current = { onError, onRefresh };

  const discover = useCallback(async () => {
    const request = ++discoveryRequest.current;
    setDiscovering(true);
    const results = await Promise.allSettled([
      performanceIpc.audioOutputs(), performanceIpc.linkInterfaces(),
      performanceIpc.midiDevices(), performanceIpc.midiProfiles(),
    ]);
    if (!alive.current || request !== discoveryRequest.current) return;
    const errors: string[] = [];
    const [audioResult, interfaceResult, devicesResult, profilesResult] = results;
    if (audioResult.status === "fulfilled") setOutputs(audioResult.value);
    else errors.push(`音声デバイス: ${String(audioResult.reason)}`);
    if (interfaceResult.status === "fulfilled") setInterfaces(interfaceResult.value);
    else errors.push(`ネットワーク: ${String(interfaceResult.reason)}`);
    if (devicesResult.status === "fulfilled") setDevices(devicesResult.value);
    else errors.push(`MIDI デバイス: ${String(devicesResult.reason)}`);
    if (profilesResult.status === "fulfilled") setProfiles(profilesResult.value);
    else errors.push(`MIDI プリセット: ${String(profilesResult.reason)}`);
    setDiscoveryErrors(errors);
    setDiscovering(false);
  }, []);

  useEffect(() => {
    alive.current = true;
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    closeButton.current?.focus();
    void discover();
    return () => {
      alive.current = false;
      discoveryRequest.current += 1;
      document.body.style.overflow = previousOverflow;
      if (previousFocus?.isConnected) previousFocus.focus({ preventScroll: true });
    };
  }, [discover]);

  const run = async (key: string, operation: () => Promise<void>, success: string) => {
    if (busy.current) return;
    busy.current = true;
    setPending(key);
    setError("");
    setMessage("");
    try {
      await operation();
      if (alive.current) setMessage(success);
      callbacks.current.onRefresh();
    } catch (failure) {
      const text = String(failure);
      if (alive.current) setError(text);
      callbacks.current.onError(text);
    } finally {
      busy.current = false;
      if (alive.current) setPending(null);
    }
  };

  const trapFocus = (event: KeyboardEvent<HTMLDivElement>) => {
    // Global deck shortcuts must not operate the mixer behind a modal.
    event.stopPropagation();
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onClose();
      return;
    }
    if (event.key !== "Tab") return;
    const controls = Array.from(dialog.current?.querySelectorAll<HTMLElement>(
      'button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), a[href], [tabindex="0"]',
    ) ?? []).filter((element) => element.tabIndex >= 0 && element.getClientRects().length > 0);
    const first = controls[0];
    const last = controls[controls.length - 1];
    if (!first || !last) {
      event.preventDefault();
      dialog.current?.focus();
    } else if (event.shiftKey && (document.activeElement === first || document.activeElement === dialog.current)) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  };

  const moveTab = (event: KeyboardEvent<HTMLButtonElement>, current: ConnectionTab) => {
    const tabs: ConnectionTab[] = ["audio", "link", "midi"];
    const index = tabs.indexOf(current);
    const next = event.key === "ArrowRight" ? tabs[(index + 1) % tabs.length]
      : event.key === "ArrowLeft" ? tabs[(index + tabs.length - 1) % tabs.length]
      : event.key === "Home" ? tabs[0] : event.key === "End" ? tabs[tabs.length - 1] : undefined;
    if (!next) return;
    event.preventDefault();
    setTab(next);
    dialog.current?.querySelector<HTMLButtonElement>(`#ctl-tab-${next}`)?.focus();
  };

  const output = audio.device_name === null ? outputs.find((device) => device.is_default) : outputs.find((device) => device.name === audio.device_name);
  const sameCueDevice = audio.cue_device_name === null || audio.cue_device_name === output?.name;
  const cueOutput = sameCueDevice ? output : outputs.find((device) => device.name === audio.cue_device_name);
  const outputChannels = output?.max_output_channels ?? 0;
  const cueChannels = cueOutput?.max_output_channels ?? 0;
  const mustStop = mixer.deck_a.state === "play" || mixer.deck_b.state === "play" || mixer.template !== null;
  const profile = profiles.find((item) => item.id === midi.profile);
  const cdjProfile = Boolean(profile?.id.startsWith("cdj-"));
  const validPair = (pair: number, channels: number) => Number.isInteger(pair) && pair >= 0 && pair + 1 < channels;
  let audioIssue = "";
  if (!output) audioIssue = "利用可能な出力デバイスを選択してください。";
  else if (audio.sample_rate !== null && !output.sample_rates.includes(audio.sample_rate)) audioIssue = "選択したデバイスで利用可能なサンプルレートを選択してください。";
  else if (audio.mode === "external") {
    if (!validPair(audio.deck_a_pair, outputChannels) || !validPair(audio.deck_b_pair, outputChannels)) audioIssue = "External には A / B 用のステレオ出力が必要です。";
    else if (Math.abs(audio.deck_a_pair - audio.deck_b_pair) < 2) audioIssue = "Deck A と Deck B の出力チャンネルが重複しています。";
  } else if (!validPair(audio.main_pair, outputChannels)) audioIssue = "MAIN のステレオ出力を選択してください。";
  else if (audio.cue_pair !== null && !validPair(audio.cue_pair, cueChannels)) audioIssue = "CUE のステレオ出力を選択してください。";
  else if (sameCueDevice && audio.cue_pair !== null && Math.abs(audio.main_pair - audio.cue_pair) < 2) audioIssue = "MAIN と CUE の出力チャンネルが重複しています。";
  const linkInterfaceAvailable = interfaces.some((item) => item.ip === link.interface_ip && item.broadcast === link.broadcast_ip && item.mac_address.every((byte, index) => byte === link.mac_address[index]));
  const linkIssue = link.enabled && !linkInterfaceAvailable ? "利用可能なネットワークインターフェースを選択してください。" : !Number.isFinite(link.latency_ms) || link.latency_ms < 0 || link.latency_ms > 1000 ? "遅延補正は 0〜1000 ms で指定してください。" : "";

  const applyAudio = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (mustStop || audioIssue) return;
    void run("audio", () => performanceIpc.configureAudio({
      ...audio,
      cue_device_name: audio.mode === "external" || audio.cue_pair === null || sameCueDevice ? null : audio.cue_device_name,
      // These are live mixer controls and may have changed while the drawer
      // was open. Applying routes must preserve their current values.
      headphone_mix: mixer.headphone_mix,
      headphone_volume: mixer.headphone_volume,
    }), "音声出力を適用しました。");
  };

  const applyMidi = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!midi.input_port || !profile) return;
    let bindings: MidiBinding[] = [];
    if (customBindings) {
      try {
        const parsed: unknown = JSON.parse(bindingText);
        if (!Array.isArray(parsed)) throw new Error("割り当ては JSON 配列で入力してください。");
        bindings = parsed as MidiBinding[];
      } catch (failure) {
        const text = `MIDI 割り当て: ${String(failure)}`;
        setError(text);
        callbacks.current.onError(text);
        return;
      }
    }
    void run("midi", () => performanceIpc.configureMidi({ ...midi, deck: cdjProfile ? midi.deck : null, bindings }), "MIDI 接続を適用しました。");
  };

  return (
    <div className="ctl-connections-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <div ref={dialog} className="ctl-connections" role="dialog" aria-modal="true" aria-labelledby="ctl-connections-title" tabIndex={-1} onKeyDown={trapFocus}>
        <header className="ctl-drawer-head">
          <div><h2 id="ctl-connections-title">接続と出力</h2><p className="ctl-muted">Audio · Pro DJ Link · MIDI</p></div>
          <button ref={closeButton} type="button" className="ctl-button" onClick={onClose} aria-label="接続設定を閉じる">閉じる ×</button>
        </header>
        <div className="ctl-drawer-tabs" role="tablist" aria-label="接続の種類">
          {(["audio", "link", "midi"] as const).map((item) => <button key={item} id={`ctl-tab-${item}`} type="button" role="tab" tabIndex={tab === item ? 0 : -1} aria-selected={tab === item} aria-controls={`ctl-panel-${item}`} className={`ctl-button${tab === item ? " is-active" : ""}`} onClick={() => setTab(item)} onKeyDown={(event) => moveTab(event, item)}>{item === "audio" ? "Audio" : item === "link" ? "Pro DJ Link" : "MIDI"}</button>)}
          <button type="button" className="ctl-button" disabled={discovering || pending !== null} onClick={() => void discover()}>{discovering ? "確認中…" : "機器を再検出"}</button>
        </div>
        <div className="ctl-drawer-content">
          {discoveryErrors.length > 0 && <div className="ctl-connection-error" role="alert">{discoveryErrors.map((text) => <p key={text}>{text}</p>)}</div>}
          {error && <p className="ctl-connection-error" role="alert">{error}</p>}
          {message && <p className="ctl-connection-message" role="status">{message}</p>}
          {tab === "audio" && <section role="tabpanel" id="ctl-panel-audio" aria-labelledby="ctl-tab-audio">
            <form onSubmit={applyAudio}>
              <div className="ctl-form-grid">
                <Field label="音声出力デバイス"><select value={audio.device_name ?? ""} onChange={(event) => setAudio({ ...audio, device_name: event.target.value || null })}>
                  <option value="">システム既定{outputs.find((item) => item.is_default) ? ` — ${outputs.find((item) => item.is_default)?.name}` : " — 利用不可"}</option>
                  {audio.device_name && !outputs.some((item) => item.name === audio.device_name) && <option value={audio.device_name} disabled>{audio.device_name} — 未接続</option>}
                  {outputs.map((item) => <option key={item.name} value={item.name}>{item.name} ({item.max_output_channels} ch)</option>)}
                </select></Field>
                <Field label="ミキシング方式"><select value={audio.mode} onChange={(event) => setAudio({ ...audio, mode: event.target.value as AudioOutputConfig["mode"] })}><option value="internal">Internal — ソフト内ミックス</option><option value="external">External — DJM でミックス</option></select></Field>
                <Field label="サンプルレート"><select value={audio.sample_rate ?? "auto"} onChange={(event) => setAudio({ ...audio, sample_rate: event.target.value === "auto" ? null : Number(event.target.value) })}>
                  <option value="auto">自動</option>
                  {audio.sample_rate !== null && !output?.sample_rates.includes(audio.sample_rate) && <option value={audio.sample_rate} disabled>{audio.sample_rate} Hz — 利用不可</option>}
                  {output?.sample_rates.map((rate) => <option key={rate} value={rate}>{rate.toLocaleString()} Hz</option>)}
                </select></Field>
                <Field label="音声バッファー (frames)"><select value={audio.buffer_frames ?? "auto"} onChange={(event) => setAudio({ ...audio, buffer_frames: event.target.value === "auto" ? null : Number(event.target.value) })}>
                  <option value="auto">デバイスの既定値</option>
                  {[32, 64, 128, 256, 512, 1024, 2048, 4096, 8192].map((frames) => <option key={frames} value={frames}>{frames}</option>)}
                </select></Field>
                {audio.mode === "internal" ? <>
                  <PairSelect label="MAIN チャンネル" value={audio.main_pair} channels={outputChannels} onChange={(value) => { if (value !== null) setAudio({ ...audio, main_pair: value }); }} />
                  <Field label="ヘッドホン CUE デバイス"><select value={sameCueDevice ? "" : audio.cue_device_name ?? ""} onChange={(event) => setAudio({ ...audio, cue_device_name: event.target.value || null })}>
                    <option value="">MAIN と同じデバイス</option>
                    {audio.cue_device_name && !sameCueDevice && !outputs.some((item) => item.name === audio.cue_device_name) && <option value={audio.cue_device_name} disabled>{audio.cue_device_name} — 未接続</option>}
                    {outputs.filter((item) => item.name !== output?.name).map((item) => <option key={item.name} value={item.name}>{item.name} ({item.max_output_channels} ch)</option>)}
                  </select></Field>
                  <PairSelect label="ヘッドホン CUE チャンネル" value={audio.cue_pair} channels={cueChannels} optional onChange={(value) => setAudio({ ...audio, cue_pair: value })} />
                </> : <>
                  <PairSelect label="Deck A チャンネル" value={audio.deck_a_pair} channels={outputChannels} onChange={(value) => { if (value !== null) setAudio({ ...audio, deck_a_pair: value }); }} />
                  <PairSelect label="Deck B チャンネル" value={audio.deck_b_pair} channels={outputChannels} onChange={(value) => { if (value !== null) setAudio({ ...audio, deck_b_pair: value }); }} />
                </>}
              </div>
              <p className="ctl-muted">{audio.mode === "external" ? "各デッキを別のステレオチャンネルへ出力します。EQ・音量・クロスフェーダー・ヘッドホンは外部ミキサーで操作してください。デッキ FX は有効です。" : "MAIN とヘッドホン CUE を別のステレオ出力に割り当てます。別デバイスの CUE も利用できます。"}</p>
              {mustStop && <p className="ctl-connection-warning">変更を適用するには A / B の再生と自動化を停止してください。</p>}
              {audioIssue && <p className="ctl-connection-warning">{audioIssue}</p>}
              <button type="submit" className="ctl-button is-active" disabled={pending !== null || discovering || mustStop || Boolean(audioIssue)}>{pending === "audio" ? "適用中…" : "音声出力を適用"}</button>
            </form>
            <div className="ctl-connection-status"><strong>現在の出力</strong><p>{performance.audio.sample_rate.toLocaleString()} Hz · {performance.audio.output_channels} ch · 推定遅延 {performance.audio.estimated_latency_ms.toFixed(1)} ms · 音声途切れ {performance.audio.underruns}</p>{performance.audio.device_lost && <p className="ctl-connection-error">音声デバイスが切断されています。</p>}{performance.audio.error && <p className="ctl-connection-error">{performance.audio.error}</p>}</div>
          </section>}

          {tab === "link" && <section role="tabpanel" id="ctl-panel-link" aria-labelledby="ctl-tab-link">
            {!performance.link.hardware_verified && <p className="ctl-connection-warning">実験的対応 · この機材構成の実機互換性は未検証です。</p>}
            <form onSubmit={(event) => { event.preventDefault(); if (!linkIssue) void run("link", () => performanceIpc.configureLink(link), "Pro DJ Link 設定を適用しました。"); }}>
              <div className="ctl-form-grid">
                <label className="ctl-check"><input type="checkbox" checked={link.enabled} onChange={(event) => setLink({ ...link, enabled: event.target.checked })} />Pro DJ Link に接続する</label>
                <label className="ctl-check"><input type="checkbox" checked={link.library_enabled} onChange={(event) => setLink({ ...link, library_enabled: event.target.checked })} />ライブラリを LAN に公開する</label>
                <Field label="ネットワークインターフェース"><select value={linkInterfaceAvailable ? link.interface_ip : ""} onChange={(event) => { const selected = interfaces.find((item) => item.ip === event.target.value); if (selected) setLink({ ...link, interface_ip: selected.ip, broadcast_ip: selected.broadcast, mac_address: [...selected.mac_address] }); }}>
                  <option value="" disabled>インターフェースを選択</option>
                  {interfaces.map((item) => <option key={`${item.name}:${item.ip}`} value={item.ip}>{item.name} — {item.ip}</option>)}
                </select></Field>
                <Field label="送信元デッキ"><select value={link.source_deck} onChange={(event) => setLink({ ...link, source_deck: event.target.value as DeckId })}><option value="A">Deck A</option><option value="B">Deck B</option></select></Field>
                <Field label="仮想プレーヤー番号"><select value={link.preferred_player ?? "auto"} onChange={(event) => setLink({ ...link, preferred_player: event.target.value === "auto" ? null : Number(event.target.value) })}><option value="auto">自動 — 空き番号を取得</option>{[1, 2, 3, 4].map((number) => <option key={number} value={number}>{number}</option>)}</select></Field>
                <Field label="音声出力の遅延補正 (ms)"><input type="number" min={0} max={1000} step={0.1} required value={Number.isFinite(link.latency_ms) ? link.latency_ms : ""} onChange={(event) => setLink({ ...link, latency_ms: event.target.value === "" ? Number.NaN : Number(event.target.value) })} /></Field>
              </div>
              {linkIssue && <p className="ctl-connection-warning">{linkIssue}</p>}
              <button type="submit" className="ctl-button is-active" disabled={pending !== null || discovering || performance.library_preparing || Boolean(linkIssue)}>{pending === "link" || performance.library_preparing ? "ライブラリを準備中…" : "Link 設定を適用"}</button>
            </form>
            <div className="ctl-connection-status"><strong>{performance.link.running ? "Link 接続中" : "Link 停止中"}</strong><p>プレーヤー {performance.link.player_number ?? "未取得"} · ソース {performance.link.source_number} · マスター {performance.link.master_number ?? "なし"} · 公開 {performance.link.library_tracks} 曲</p><p className="ctl-muted">{performance.link.capability}</p>
              <div className="ctl-connection-actions"><button type="button" className="ctl-button" disabled={pending !== null || !performance.link.running || performance.link.player_number === null} onClick={() => void run("master", performanceIpc.requestLinkMaster, "マスターへの切り替えを要求しました。")}>{pending === "master" ? "要求中…" : "マスターを要求"}</button><button type="button" className="ctl-button" disabled={pending !== null || performance.library_preparing || !performance.link.running || !performance.link_config.library_enabled} onClick={() => void run("library", performanceIpc.refreshLinkLibrary, "公開ライブラリを更新しました。")}>{pending === "library" || performance.library_preparing ? "ライブラリを準備中…" : "公開ライブラリを更新"}</button></div>
              {performance.link.last_error && <p className="ctl-connection-error">{performance.link.last_error}</p>}
              {performance.link.devices.length === 0 ? <p className="ctl-muted">LAN 上の機器はまだ見つかっていません。</p> : performance.link.devices.map((device) => <div className="ctl-connection-row" key={`${device.device_number}:${device.ip}`}><div><strong>{device.device_number} · {device.name}</strong><small>{device.ip}</small></div><span>{device.bpm === null ? "—" : device.bpm.toFixed(1)} BPM · {device.playing ? "PLAY" : "STOP"}{device.master ? " · MASTER" : ""}{device.synced ? " · SYNC" : ""}</span></div>)}
            </div>
            {performance.catalog_issues.length > 0 && <details className="ctl-connection-issues"><summary>公開できなかった曲 ({performance.catalog_issues.length})</summary>{performance.catalog_issues.map((issue, index) => <p key={`${issue.track_id}:${index}`}><strong>{issue.title || issue.track_id}</strong> — {issue.message}</p>)}</details>}
          </section>}

          {tab === "midi" && <section role="tabpanel" id="ctl-panel-midi" aria-labelledby="ctl-tab-midi">
            <form onSubmit={applyMidi}>
              <div className="ctl-form-grid">
                <Field label="MIDI 入力"><select value={midi.input_port} required onChange={(event) => setMidi({ ...midi, input_port: event.target.value })}><option value="" disabled>コントローラーを選択</option>{devices.inputs.map((port) => <option key={port.id} value={port.id}>{port.name}</option>)}</select></Field>
                <Field label="MIDI 出力 / LED"><select value={midi.output_port ?? ""} onChange={(event) => setMidi({ ...midi, output_port: event.target.value || null })}><option value="">出力なし</option>{devices.outputs.map((port) => <option key={port.id} value={port.id}>{port.name}</option>)}</select></Field>
                <Field label="機種プリセット"><select value={midi.profile} required onChange={(event) => { setMidi({ ...midi, profile: event.target.value, deck: null }); if (!customBindings) setBindingText(JSON.stringify(profiles.find((item) => item.id === event.target.value)?.bindings ?? [], null, 2)); }}><option value="" disabled>プリセットを選択</option>{profiles.map((item) => <option key={item.id} value={item.id}>{item.name}{item.experimental ? " (実験的)" : ""}</option>)}</select></Field>
                <Field label="操作するデッキ (CDJ)"><select value={midi.deck ?? "auto"} disabled={!cdjProfile} onChange={(event) => setMidi({ ...midi, deck: event.target.value === "auto" ? null : event.target.value as DeckId })}><option value="auto">プリセットに従う</option><option value="A">Deck A</option><option value="B">Deck B</option></select></Field>
              </div>
              {devices.inputs.length === 0 && !discovering && <p className="ctl-muted">MIDI 入力がありません。コントローラーを接続し、機器を再検出してください。</p>}
              {profile && <div className="ctl-profile-info"><p>{profile.bindings.length} 個の入力割り当て · {profile.leds.length} 個の LED 割り当て{profile.experimental ? " · 実験的対応" : ""}</p>{profile.warnings.map((warning) => <p key={warning} className="ctl-connection-warning">{warning}</p>)}{profile.sources.length > 0 && <p className="ctl-profile-sources">仕様資料: {profile.sources.map((source, index) => /^https?:\/\//iu.test(source) ? <a key={source} href={source} target="_blank" rel="noreferrer noopener">資料 {index + 1}</a> : <span key={source}>{source}</span>)}</p>}</div>}
              <details className="ctl-midi-advanced"><summary>詳細: カスタム MIDI 割り当て</summary><label className="ctl-check"><input type="checkbox" checked={customBindings} onChange={(event) => setCustomBindings(event.target.checked)} />JSON で入力割り当てを指定する</label><p className="ctl-muted">JSON 配列で入力してください。空配列は選択したプリセットを使用します。デッキ選択は CDJ のみに適用されます。</p><textarea aria-label="カスタム MIDI 割り当て JSON" value={bindingText} disabled={!customBindings} onChange={(event) => setBindingText(event.target.value)} rows={10} spellCheck={false} /><button type="button" className="ctl-button" disabled={!profile} onClick={() => { setCustomBindings(true); setBindingText(JSON.stringify(profile?.bindings ?? [], null, 2)); }}>プリセットから読み込む</button></details>
              <button type="submit" className="ctl-button is-active" disabled={pending !== null || discovering || !devices.inputs.some((port) => port.id === midi.input_port) || !profile}>{pending === "midi" ? "接続中…" : "MIDI を接続 / 更新"}</button>
            </form>
            <div className="ctl-connection-status"><strong>現在の接続</strong><p className="ctl-muted">破棄された MIDI メッセージ: {performance.midi.dropped_messages}</p>{performance.midi.error && <p className="ctl-connection-error">{performance.midi.error}</p>}{performance.midi.connections.length === 0 && <p className="ctl-muted">コントローラーは未接続です。</p>}{performance.midi.connections.map((connection) => <div className="ctl-connection-row" key={connection.config.input_port}><div><strong>{connection.profile_name} · {connection.connected ? "接続中" : "切断"}</strong><small>{connection.config.input_port}{connection.config.deck ? ` · Deck ${connection.config.deck}` : ""}</small><small>{bindingCount(connection.config, profiles)} 個の入力割り当て · LED {connection.config.output_port ?? "出力なし"}</small>{connection.error && <p className="ctl-connection-error">{connection.error}</p>}</div><button type="button" className="ctl-button" disabled={pending !== null} onClick={() => void run(`disconnect:${connection.config.input_port}`, () => performanceIpc.disconnectMidi(connection.config.input_port), "MIDI 接続を解除しました。")}>{pending === `disconnect:${connection.config.input_port}` ? "解除中…" : "接続解除"}</button></div>)}</div>
          </section>}
        </div>
      </div>
    </div>
  );
}
