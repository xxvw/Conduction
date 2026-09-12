import { useCallback, useEffect, useRef, useState } from "react";

import { ipc } from "@/lib/ipc";

export interface AudioSettingsState {
  devices: string[];
  mainOutput: string | null;
  cueOutput: string | null;
  loading: boolean;
  saving: boolean;
  error: string | null;
}

/** Device changes apply immediately after the backend validates stopped playback. */
export function useAudioSettings() {
  const [state, setState] = useState<AudioSettingsState>({
    devices: [],
    mainOutput: null,
    cueOutput: null,
    loading: true,
    saving: false,
    error: null,
  });
  const mounted = useRef(false);
  const pending = useRef<Promise<void>>(Promise.resolve());
  const queued = useRef(0);

  useEffect(() => {
    mounted.current = true;
    let cancelled = false;
    void (async () => {
      try {
        const [devices, settings] = await Promise.all([
          ipc.listAudioDevices(),
          ipc.getSettings(),
        ]);
        if (cancelled) return;
        setState((current) => ({
          ...current,
          devices,
          mainOutput: settings.audio_main_output ?? null,
          cueOutput: settings.audio_cue_output ?? null,
          loading: false,
        }));
      } catch (error) {
        if (!cancelled) {
          setState((current) => ({ ...current, loading: false, error: String(error) }));
        }
      }
    })();
    return () => {
      cancelled = true;
      mounted.current = false;
    };
  }, []);

  const update = useCallback((intent: "main" | "cue", name: string | null) => {
    queued.current += 1;
    setState((current) => ({ ...current, saving: true, error: null }));
    const operation = pending.current.then(async () => {
      try {
        // Read fresh settings so another settings panel cannot be reverted by this form.
        const current = await ipc.getSettings();
        const next = {
          ...current,
          [intent === "main" ? "audio_main_output" : "audio_cue_output"]: name,
        };
        await ipc.saveSettings(next, intent);
        const saved = await ipc.getSettings();
        if (mounted.current) {
          setState((value) => ({
            ...value,
            mainOutput: saved.audio_main_output ?? null,
            cueOutput: saved.audio_cue_output ?? null,
            error: null,
          }));
        }
      } catch (error) {
        // Keep the confirmed selection when the device, routes, or playback state reject it.
        if (mounted.current) setState((current) => ({ ...current, error: String(error) }));
      } finally {
        queued.current -= 1;
        if (mounted.current && queued.current === 0) {
          setState((current) => ({ ...current, saving: false }));
        }
      }
    });
    pending.current = operation;
    return operation;
  }, []);

  const setMain = useCallback((name: string | null) => update("main", name), [update]);
  const setCue = useCallback((name: string | null) => update("cue", name), [update]);

  return { state, setMain, setCue };
}
