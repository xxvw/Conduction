import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { HoldButton } from "./ControllerControls";

function deferred() {
  let resolve!: () => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<void>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

beforeEach(() => {
  // jsdom has no pointer capture. Keep real event dispatch and React handlers.
  class TestPointerEvent extends MouseEvent {
    readonly pointerId: number;
    constructor(type: string, init: MouseEventInit & { pointerId?: number } = {}) {
      super(type, init);
      this.pointerId = init.pointerId ?? 1;
    }
  }
  vi.stubGlobal("PointerEvent", TestPointerEvent);
  Object.defineProperty(HTMLElement.prototype, "setPointerCapture", { configurable: true, value: vi.fn() });
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe("HoldButton transport gate", () => {
  it.each(["pointer cancel", "lost capture", "window blur", "unmount"] as const)(
    "queues release after a pending press on %s",
    async (releaseCause) => {
      const press = deferred();
      const onHold = vi.fn((pressed: boolean) => pressed ? press.promise : Promise.resolve());
      const onError = vi.fn();
      const view = render(<HoldButton onHold={onHold} onError={onError}>Transport Cue</HoldButton>);
      const button = screen.getByRole("button", { name: "Transport Cue" });

      fireEvent.pointerDown(button, { button: 0, pointerId: 1 });
      await waitFor(() => expect(onHold.mock.calls).toEqual([[true]]));
      if (releaseCause === "pointer cancel") fireEvent.pointerCancel(button, { pointerId: 1 });
      if (releaseCause === "lost capture") fireEvent.lostPointerCapture(button, { pointerId: 1 });
      if (releaseCause === "window blur") fireEvent(window, new Event("blur"));
      if (releaseCause === "unmount") view.unmount();

      await act(async () => { await Promise.resolve(); });
      expect(onHold.mock.calls).toEqual([[true]]);
      await act(async () => { press.resolve(); });
      await waitFor(() => expect(onHold.mock.calls).toEqual([[true], [false]]));

      // Browsers may send several release events for the same gesture.
      fireEvent.pointerUp(button, { pointerId: 1 });
      fireEvent.pointerCancel(button, { pointerId: 1 });
      fireEvent(window, new Event("blur"));
      view.unmount();
      await act(async () => { await Promise.resolve(); });
      expect(onHold.mock.calls).toEqual([[true], [false]]);
      expect(onError).not.toHaveBeenCalled();
    },
  );

  it.each([" ", "Enter"])("ignores keyboard repeats for %j and releases once", async (key) => {
    const onHold = vi.fn(async (_pressed: boolean) => {});
    const parentKeyDown = vi.fn();
    const parentKeyUp = vi.fn();
    render(<div onKeyDown={parentKeyDown} onKeyUp={parentKeyUp}>
      <HoldButton onHold={onHold} onError={vi.fn()}>Nudge</HoldButton>
    </div>);
    const button = screen.getByRole("button", { name: "Nudge" });

    fireEvent.keyDown(button, { key, repeat: true });
    await act(async () => { await Promise.resolve(); });
    expect(onHold).not.toHaveBeenCalled();
    fireEvent.keyDown(button, { key });
    fireEvent.keyDown(button, { key, repeat: true });
    fireEvent.keyDown(button, { key, repeat: true });
    fireEvent.keyUp(button, { key });
    fireEvent.keyUp(button, { key });

    await waitFor(() => expect(onHold.mock.calls).toEqual([[true], [false]]));
    expect(parentKeyDown).not.toHaveBeenCalled();
    expect(parentKeyUp).not.toHaveBeenCalled();
  });

  it("still sends release when the pending press fails", async () => {
    const press = deferred();
    const onHold = vi.fn((pressed: boolean) => pressed ? press.promise : Promise.resolve());
    const onError = vi.fn();
    render(<HoldButton onHold={onHold} onError={onError}>Jog touch</HoldButton>);
    const button = screen.getByRole("button", { name: "Jog touch" });

    fireEvent.pointerDown(button, { button: 0 });
    await waitFor(() => expect(onHold).toHaveBeenCalledWith(true));
    fireEvent.pointerCancel(button);
    await act(async () => { press.reject(new Error("Device disconnected")); });

    await waitFor(() => expect(onHold.mock.calls).toEqual([[true], [false]]));
    expect(onError).toHaveBeenCalledWith("Error: Device disconnected");
  });

  it("reports a release failure and accepts the next transport gesture", async () => {
    const onHold = vi.fn(async (_pressed: boolean) => {})
      .mockResolvedValueOnce(undefined)
      .mockRejectedValueOnce(new Error("Release failed"));
    const onError = vi.fn();
    render(<HoldButton onHold={onHold} onError={onError}>Transport Cue</HoldButton>);
    const button = screen.getByRole("button", { name: "Transport Cue" });

    fireEvent.keyDown(button, { key: "Enter" });
    fireEvent.keyUp(button, { key: "Enter" });
    await waitFor(() => expect(onError).toHaveBeenCalledWith("Error: Release failed"));
    fireEvent.keyDown(button, { key: "Enter" });
    fireEvent.keyUp(button, { key: "Enter" });

    await waitFor(() => expect(onHold.mock.calls).toEqual([[true], [false], [true], [false]]));
  });
});
