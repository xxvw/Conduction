import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ipc, type SetlistDto } from "@/lib/ipc";
import type { DeckId } from "@/types/mixer";
import type { PerformanceBrowser } from "@/types/performance";
import type { TrackSummary } from "@/types/track";
import { ControllerLibrary } from "./ControllerLibrary";

vi.mock("@/lib/ipc", () => ({ ipc: { listSetlists: vi.fn() } }));

function track(id: string, title: string, extra: Partial<TrackSummary> = {}): TrackSummary {
  return {
    id, path: `/music/${id}.wav`, title, artist: "Conduction", album: "Sessions",
    genre: "House", duration_sec: 180, bpm: 124, key: "8A", energy: 0.5,
    beatgrid_verified: true, analyzed: true, ...extra,
  };
}

const tracks: TrackSummary[] = [
  track("opening", "Opening", { artist: "First Artist", key: "7A" }),
  track("night", "夜の波", { artist: "東京", album: "空間" }),
  track("moon", "Moon", { artist: "Second Artist", key: "9B" }),
];

const setlist: SetlistDto = {
  id: "night-set",
  name: "深夜セット",
  entries: [
    { id: "entry-night-1", track_id: "night" },
    { id: "entry-missing", track_id: "missing-track" },
    { id: "entry-opening", track_id: "opening" },
    { id: "entry-night-2", track_id: "night" },
  ],
};

function deferred() {
  let resolve!: () => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<void>((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}

function visibleTitles() {
  return within(screen.getByRole("table")).getAllByRole("row").slice(1)
    .map((row) => within(row).getAllByRole("cell")[0]?.textContent);
}

async function waitForSetlists() {
  await screen.findByRole("option", { name: "深夜セット" });
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(ipc.listSetlists).mockResolvedValue([setlist]);
});

afterEach(cleanup);

describe("ControllerLibrary", () => {
  it("uses actual setlist order, preserves repeated tracks and omits missing files", async () => {
    render(<ControllerLibrary tracks={tracks} onLoadToDeck={vi.fn()} onError={vi.fn()} />);
    await waitForSetlists();

    expect(visibleTitles()).toEqual(["Opening", "夜の波", "Moon"]);
    fireEvent.change(screen.getByRole("combobox", { name: "ライブラリまたはセットリスト" }), {
      target: { value: setlist.id },
    });

    expect(visibleTitles()).toEqual(["夜の波", "Opening", "夜の波"]);
    expect(screen.getByText("3 tracks")).toBeTruthy();
  });

  it("matches Japanese title, artist and album terms and explains an empty search", async () => {
    render(<ControllerLibrary tracks={tracks} onLoadToDeck={vi.fn()} onError={vi.fn()} />);
    await waitForSetlists();
    const search = screen.getByRole("searchbox");

    fireEvent.change(search, { target: { value: "夜 東京 空間" } });
    expect(visibleTitles()).toEqual(["夜の波"]);
    fireEvent.change(search, { target: { value: "9b" } });
    expect(visibleTitles()).toEqual(["Moon"]);
    fireEvent.change(search, { target: { value: "見つからない曲" } });

    expect(visibleTitles()).toEqual([]);
    expect(screen.getByText("検索条件に一致する曲がありません。")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /を Deck [AB] に読み込み/ })).toBeNull();
  });

  it("awaits each deck independently and reports a decode failure without a false success", async () => {
    const deckA = deferred();
    const deckB = deferred();
    const onLoad = vi.fn((deck: DeckId) => deck === "A" ? deckA.promise : deckB.promise);
    const onError = vi.fn();
    render(<ControllerLibrary tracks={tracks} onLoadToDeck={onLoad} onError={onError} />);
    await waitForSetlists();
    const openingA = screen.getByRole<HTMLButtonElement>("button", { name: "Opening を Deck A に読み込み" });
    const nightA = screen.getByRole<HTMLButtonElement>("button", { name: "夜の波 を Deck A に読み込み" });
    const nightB = screen.getByRole<HTMLButtonElement>("button", { name: "夜の波 を Deck B に読み込み" });

    fireEvent.click(openingA);
    expect(onLoad).toHaveBeenCalledWith("A", "/music/opening.wav");
    expect(openingA.disabled).toBe(true);
    expect(nightA.disabled).toBe(true);
    expect(nightB.disabled).toBe(false);
    expect(openingA.getAttribute("aria-busy")).toBe("true");
    expect(screen.getByRole("status").textContent).toContain("読み込み中");
    expect(screen.getByRole("status").textContent).not.toContain("読み込みました");

    fireEvent.click(nightB);
    expect(onLoad).toHaveBeenCalledTimes(2);
    expect(nightB.disabled).toBe(true);
    await act(async () => { deckA.reject(new Error("decoder rejected corrupt audio")); });

    expect(onError).toHaveBeenCalledTimes(1);
    expect(onError.mock.calls[0]?.[0]).toContain("decoder rejected corrupt audio");
    expect(screen.getByRole("status").textContent).toBe("Deck A: 読み込みに失敗しました");
    expect(openingA.disabled).toBe(false);
    expect(nightB.disabled).toBe(true);
    expect(openingA.getAttribute("aria-busy")).toBe("false");

    await act(async () => { deckB.resolve(); });
    expect(nightB.disabled).toBe(false);
    expect(screen.getByRole("status").textContent).toBe("Deck B: 夜の波 を読み込みました");
    expect(screen.queryByText("Deck A: Opening を読み込みました")).toBeNull();
  });

  it("reflects MIDI selection while preserving search text during keyboard input", async () => {
    const props = { tracks, onLoadToDeck: vi.fn(), onError: vi.fn() };
    const browser: PerformanceBrowser = { query: "", selected_track_id: "night", playlist_id: null };
    const view = render(<ControllerLibrary {...props} browser={browser} />);
    await waitForSetlists();
    expect(screen.getByText("夜の波").closest("tr")?.classList.contains("is-selected")).toBe(true);
    const search = screen.getByRole<HTMLInputElement>("searchbox");

    fireEvent.focus(search);
    fireEvent.change(search, { target: { value: "夜" } });
    view.rerender(<ControllerLibrary {...props} browser={{ ...browser, query: "Opening", selected_track_id: "opening" }} />);

    expect(search.value).toBe("夜");
    expect(visibleTitles()).toEqual(["夜の波"]);
    expect(screen.getByText("夜の波").closest("tr")?.classList.contains("is-selected")).toBe(true);

    fireEvent.blur(search);
    view.rerender(<ControllerLibrary {...props} browser={{ ...browser, query: "Moon", selected_track_id: "moon" }} />);
    expect(search.value).toBe("Moon");
    expect(visibleTitles()).toEqual(["Moon"]);
    expect(screen.getByText("Moon").closest("tr")?.classList.contains("is-selected")).toBe(true);
  });

  it("publishes the selected visible track with the active playlist and search", async () => {
    const onBrowserChange = vi.fn<(browser: PerformanceBrowser) => Promise<void>>().mockResolvedValue(undefined);
    render(<ControllerLibrary tracks={tracks} onLoadToDeck={vi.fn()} onError={vi.fn()} onBrowserChange={onBrowserChange} />);
    await waitForSetlists();

    fireEvent.change(screen.getByRole("combobox", { name: "ライブラリまたはセットリスト" }), {
      target: { value: setlist.id },
    });
    await waitFor(() => expect(onBrowserChange).toHaveBeenLastCalledWith({
      query: "", selected_track_id: "night", playlist_id: setlist.id,
    }));

    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "Opening" } });
    await waitFor(() => expect(onBrowserChange).toHaveBeenLastCalledWith({
      query: "Opening", selected_track_id: "opening", playlist_id: setlist.id,
    }));

    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "no matching track" } });
    await waitFor(() => expect(onBrowserChange).toHaveBeenLastCalledWith({
      query: "no matching track", selected_track_id: null, playlist_id: setlist.id,
    }));
  });

  it("loads once on Enter after keyboard selection and blocks global shortcuts and key repeat", async () => {
    const loaded = deferred();
    const onLoad = vi.fn(() => loaded.promise);
    const globalShortcut = vi.fn();
    window.addEventListener("keydown", globalShortcut);
    try {
      render(<ControllerLibrary tracks={tracks} onLoadToDeck={onLoad} onError={vi.fn()} />);
      await waitForSetlists();
      const list = screen.getByRole("region", { name: /^曲一覧/ });
      fireEvent.keyDown(list, { key: "ArrowDown" });
      fireEvent.keyDown(list, { key: "ArrowRight" });
      fireEvent.keyDown(list, { key: "Enter" });

      expect(onLoad).toHaveBeenCalledTimes(1);
      expect(onLoad).toHaveBeenCalledWith("B", "/music/night.wav");
      expect(globalShortcut).not.toHaveBeenCalled();

      await act(async () => { loaded.resolve(); });
      fireEvent.keyDown(list, { key: "Enter", repeat: true });
      expect(onLoad).toHaveBeenCalledTimes(1);
      expect(globalShortcut).not.toHaveBeenCalled();
    } finally {
      window.removeEventListener("keydown", globalShortcut);
    }
  });
});
