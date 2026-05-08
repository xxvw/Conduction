import { useCallback, useEffect, useRef, useState } from "react";

import { ipc, type AppSettings } from "@/lib/ipc";

export interface AudioSettingsState {
  devices: string[];
  mainOutput: string | null;
  cueOutput: string | null;
  loading: boolean;
}

/**
 * Audio 出力デバイス一覧と現在の選択を読み書きする。
 *
 * バックエンドの settings.toml に `audio_main_output` / `audio_cue_output`
 * として保存される。設定変更はアプリ再起動後に audio engine が読み直す。
 */
export function useAudioSettings() {
  const [state, setState] = useState<AudioSettingsState>({
    devices: [],
    mainOutput: null,
    cueOutput: null,
    loading: true,
  });
  const settingsRef = useRef<AppSettings | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [devices, settings] = await Promise.all([
          ipc.listAudioDevices(),
          ipc.getSettings(),
        ]);
        if (cancelled) return;
        settingsRef.current = settings;
        setState({
          devices,
          mainOutput: settings.audio_main_output ?? null,
          cueOutput: settings.audio_cue_output ?? null,
          loading: false,
        });
      } catch (e) {
        console.warn("failed to load audio settings:", e);
        if (!cancelled) {
          setState((s) => ({ ...s, loading: false }));
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const update = useCallback(
    async (patch: Partial<AppSettings>) => {
      const current = settingsRef.current ?? (await ipc.getSettings());
      const next: AppSettings = { ...current, ...patch };
      settingsRef.current = next;
      await ipc.saveSettings(next);
      setState((s) => ({
        ...s,
        mainOutput: next.audio_main_output ?? null,
        cueOutput: next.audio_cue_output ?? null,
      }));
    },
    [],
  );

  const setMain = useCallback(
    (name: string | null) => update({ audio_main_output: name }),
    [update],
  );
  const setCue = useCallback(
    (name: string | null) => update({ audio_cue_output: name }),
    [update],
  );

  return { state, setMain, setCue };
}
