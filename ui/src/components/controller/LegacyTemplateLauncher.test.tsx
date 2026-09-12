import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ipc, type BuiltInTarget, type TemplateFull, type TemplatePreset } from "@/lib/ipc";
import { TemplateLauncher } from "@/components/templates/TemplateLauncher";

vi.mock("@/lib/ipc", () => ({ ipc: { getTemplatePreset: vi.fn() } }));

const mix: TemplatePreset = { id: "preset.mix", name: "Crossfade", duration_beats: 32, kind: "builtin" };
const echo: TemplatePreset = { id: "user.echo", name: "Echo Tail", duration_beats: 16, kind: "user" };
const reverb: TemplatePreset = { id: "preset.reverb", name: "Reverb Tail", duration_beats: 8, kind: "builtin" };

function full(preset: TemplatePreset, targets: BuiltInTarget[], source?: string): TemplateFull {
  return {
    id: preset.id, name: preset.name, duration_beats: preset.duration_beats, source,
    tracks: targets.map((target) => ({
      target,
      keyframes: [{ position: { kind: "beats", value: 0 }, value: 0.5, curve: "linear" }],
    })),
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}

beforeEach(() => { vi.mocked(ipc.getTemplatePreset).mockReset(); });
afterEach(cleanup);

describe("legacy TemplateLauncher in External mixing mode", () => {
  it("selects presets arriving asynchronously and falls back when the selection disappears without Internal lookups", () => {
    const onStart = vi.fn();
    const view = render(<TemplateLauncher presets={[]} currentBpm={128} onStart={onStart} />);
    expect(screen.queryByRole("combobox")).toBeNull();

    view.rerender(<TemplateLauncher presets={[mix, echo]} currentBpm={128} onStart={onStart} />);
    const select = screen.getByRole<HTMLSelectElement>("combobox");
    expect(select.value).toBe(mix.id);
    fireEvent.change(select, { target: { value: echo.id } });
    view.rerender(<TemplateLauncher presets={[echo, reverb]} currentBpm={128} onStart={onStart} />);
    expect(select.value).toBe(echo.id);

    view.rerender(<TemplateLauncher presets={[reverb]} currentBpm={128} onStart={onStart} />);
    expect(select.value).toBe(reverb.id);
    fireEvent.click(screen.getByRole("button", { name: /Start/ }));
    expect(onStart).toHaveBeenCalledWith(reverb.id, 128, false);
    expect(ipc.getTemplatePreset).not.toHaveBeenCalled();
  });

  it("keeps unknown presets disabled until checked and then selects the first FX-only result", async () => {
    const mixResult = deferred<TemplateFull>();
    const echoResult = deferred<TemplateFull>();
    vi.mocked(ipc.getTemplatePreset).mockImplementation((id) => id === mix.id ? mixResult.promise : echoResult.promise);
    const onStart = vi.fn();
    render(<TemplateLauncher presets={[mix, echo]} currentBpm={126} externalMixer onStart={onStart} />);

    const start = screen.getByRole<HTMLButtonElement>("button", { name: /Start/ });
    expect(start.disabled).toBe(true);
    expect(screen.getByRole("status").textContent).toContain("確認中");
    expect(screen.getByRole<HTMLOptionElement>("option", { name: /Echo Tail/ }).disabled).toBe(true);

    await act(async () => { mixResult.resolve(full(mix, [{ type: "crossfader" }])); });
    expect(start.disabled).toBe(true);
    await act(async () => {
      echoResult.resolve(full(echo, [
        { type: "deck_echo_wet", deck: "A" }, { type: "deck_reverb_wet", deck: "B" },
      ], "-- compiled Lua FX template"));
    });

    expect(screen.getByRole<HTMLSelectElement>("combobox").value).toBe(echo.id);
    expect(screen.getByRole<HTMLOptionElement>("option", { name: /Crossfade/ }).disabled).toBe(true);
    expect(screen.getByRole<HTMLOptionElement>("option", { name: /Echo Tail/ }).disabled).toBe(false);
    expect(start.disabled).toBe(false);
    fireEvent.click(screen.getByRole("checkbox", { name: "B→A" }));
    fireEvent.click(start);
    expect(onStart).toHaveBeenCalledExactlyOnceWith(echo.id, 126, true);
  });

  const unsupportedTargets: BuiltInTarget[] = [
    { type: "crossfader" }, { type: "master_volume" }, { type: "deck_volume", deck: "A" },
    { type: "deck_eq_low", deck: "A" }, { type: "deck_eq_mid", deck: "B" },
    { type: "deck_eq_high", deck: "A" }, { type: "deck_filter", deck: "B" },
  ];
  it.each(unsupportedTargets)("rejects a template containing $type even alongside deck FX", async (target) => {
    vi.mocked(ipc.getTemplatePreset).mockResolvedValue(full(mix, [
      { type: "deck_echo_wet", deck: "A" }, target,
    ]));
    const onStart = vi.fn();
    render(<TemplateLauncher presets={[mix]} currentBpm={125} externalMixer onStart={onStart} />);
    await waitFor(() => expect(screen.getByRole("status").textContent).toContain("対応の FX テンプレートがありません"));

    expect(screen.getByRole<HTMLOptionElement>("option", { name: /Crossfade/ }).disabled).toBe(true);
    const start = screen.getByRole<HTMLButtonElement>("button", { name: /Start/ });
    expect(start.disabled).toBe(true);
    fireEvent.click(start);
    expect(onStart).not.toHaveBeenCalled();
  });

  it("preserves a selected compatible preset when changing from Internal to External", async () => {
    vi.mocked(ipc.getTemplatePreset).mockImplementation(async (id) => id === echo.id
      ? full(echo, [{ type: "deck_echo_wet", deck: "A" }])
      : full(reverb, [{ type: "deck_reverb_wet", deck: "B" }]));
    const presets = [echo, reverb];
    const onStart = vi.fn();
    const view = render(<TemplateLauncher presets={presets} currentBpm={120} onStart={onStart} />);
    fireEvent.change(screen.getByRole("combobox"), { target: { value: reverb.id } });

    view.rerender(<TemplateLauncher presets={presets} currentBpm={120} externalMixer onStart={onStart} />);
    await waitFor(() => expect(screen.getByRole<HTMLButtonElement>("button", { name: /Start/ }).disabled).toBe(false));
    expect(screen.getByRole<HTMLSelectElement>("combobox").value).toBe(reverb.id);
  });

  it("shows lookup errors and leaves failed presets disabled while a valid FX preset works", async () => {
    vi.mocked(ipc.getTemplatePreset).mockImplementation(async (id) => {
      if (id === mix.id) throw new Error("template file unreadable");
      return full(echo, [{ type: "deck_echo_wet", deck: "A" }]);
    });
    render(<TemplateLauncher presets={[mix, echo]} currentBpm={124} externalMixer onStart={vi.fn()} />);
    expect((await screen.findByRole("alert")).textContent).toContain("template file unreadable");
    expect(screen.getByRole<HTMLOptionElement>("option", { name: /Crossfade.*確認失敗/ }).disabled).toBe(true);
    expect(screen.getByRole<HTMLSelectElement>("combobox").value).toBe(echo.id);
    expect(screen.getByRole<HTMLButtonElement>("button", { name: /Start/ }).disabled).toBe(false);
  });

  it("ignores an old compatible response after a revised preset is found incompatible", async () => {
    const oldResult = deferred<TemplateFull>();
    vi.mocked(ipc.getTemplatePreset)
      .mockReturnValueOnce(oldResult.promise)
      .mockResolvedValueOnce(full(echo, [{ type: "master_volume" }]));
    const onStart = vi.fn();
    const view = render(<TemplateLauncher presets={[echo]} currentBpm={124} externalMixer onStart={onStart} />);
    view.rerender(<TemplateLauncher presets={[{ ...echo, name: "Revised Echo" }]} currentBpm={124} externalMixer onStart={onStart} />);
    await waitFor(() => expect(screen.getByRole("status").textContent).toContain("対応の FX テンプレートがありません"));

    await act(async () => { oldResult.resolve(full(echo, [{ type: "deck_echo_wet", deck: "A" }])); });
    expect(screen.getByRole<HTMLButtonElement>("button", { name: /Start/ }).disabled).toBe(true);
    expect(screen.getByRole<HTMLOptionElement>("option", { name: /Revised Echo/ }).disabled).toBe(true);
    expect(onStart).not.toHaveBeenCalled();
  });

  it("keeps BPM validation when an External FX template is compatible", async () => {
    vi.mocked(ipc.getTemplatePreset).mockResolvedValue(full(echo, [{ type: "deck_echo_wet", deck: "A" }]));
    const presets = [echo];
    const onStart = vi.fn();
    const view = render(<TemplateLauncher presets={presets} currentBpm={0} externalMixer onStart={onStart} />);
    await waitFor(() => expect(screen.getByRole<HTMLSelectElement>("combobox").value).toBe(echo.id));
    expect(screen.getByRole<HTMLButtonElement>("button", { name: /Start/ }).disabled).toBe(true);
    view.rerender(<TemplateLauncher presets={presets} currentBpm={Infinity} externalMixer onStart={onStart} />);
    expect(screen.getByRole<HTMLButtonElement>("button", { name: /Start/ }).disabled).toBe(true);
    view.rerender(<TemplateLauncher presets={presets} currentBpm={128} externalMixer onStart={onStart} />);
    expect(screen.getByRole<HTMLButtonElement>("button", { name: /Start/ }).disabled).toBe(false);
    expect(ipc.getTemplatePreset).toHaveBeenCalledTimes(1);
  });
});
