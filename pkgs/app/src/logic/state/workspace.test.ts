import { createStore } from "jotai";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  type Character,
  finishGeneration,
  generateTake,
  prepareGeneration,
  removeTake,
  type Take,
} from "../lib/api.ts";
import { DEFAULT_PREFERENCES, MAX_SCRIPT_ROWS, newRow } from "../lib/script.ts";
import { busyAtom, errorAtom, readyAtom } from "./app.ts";
import {
  deleteTakeAtom,
  deletingTakeAtom,
  generateAllRowsAtom,
  generateSelectedRowAtom,
  generationErrorsAtom,
  runningRowAtom,
  selectedRowTakesAtom,
  selectedTakeAtom,
  stopGenerationAtom,
  stoppingAtom,
  takesAtom,
} from "./audio.ts";
import { activeCharacterAtom, characterSavedAtom, charactersAtom, editingCharacterAtom } from "./characters.ts";
import {
  appendCharacterRowAtom,
  appendTakeRowAtom,
  characterRemovedAtom,
  editSelectedRowAtom,
  focusRowAtom,
  insertSelectedRowAtom,
  moveSelectedRowAtom,
  removeSelectedRowAtom,
  reorderRowsAtom,
  rowAtomsAtom,
  rowsAtom,
  selectedCharacterAtom,
  selectedRowAtom,
  selectedRowIdAtom,
  selectedRowIndexAtom,
  selectRowAtom,
} from "./script.ts";

vi.mock("../lib/api.ts", async (original) => ({
  ...(await original<typeof import("../lib/api.ts")>()),
  generateTake: vi.fn(),
  prepareGeneration: vi.fn(),
  finishGeneration: vi.fn(),
  removeTake: vi.fn(),
}));
beforeEach(() => {
  vi.mocked(prepareGeneration).mockReset().mockResolvedValue("batch");
  vi.mocked(finishGeneration).mockReset().mockResolvedValue(undefined);
});
const subscriptions: Array<() => void> = [];
afterEach(() => {
  for (const unsubscribe of subscriptions.splice(0)) unsubscribe();
  vi.restoreAllMocks();
  vi.mocked(generateTake).mockReset();
  vi.mocked(removeTake).mockReset();
});

const characters: Character[] = ["mf", "small", "anime"].map((model) => ({
  id: model,
  name: model,
  description: "声の説明",
  voice: null,
  color: "#41a2ec",
  sample_text: "こんにちは。",
}));
const makeStore = () => {
  const store = createStore();
  subscriptions.push(store.sub(rowsAtom, () => {}));
  store.set(readyAtom, true);
  store.set(charactersAtom, characters);
  store.set(
    rowsAtom,
    characters.map((character) => ({
      ...newRow(character.id, { ...DEFAULT_PREFERENCES, seed: "19" }),
      text: character.name,
    })),
  );
  return store;
};
const takeFor = (rowId: string, id = "take-1", seed = 19): Take => ({
  id,
  row_id: rowId,
  character_id: "mf",
  character_name: "mf",
  text: "mf",
  description: "声の説明",
  additional_prompt: "",
  voice: null,
  model: "mf",
  seed: String(seed),
  num_steps: 4,
  audio_seconds: 1,
  elapsed_ms: 10,
  created_at: 1,
});
const deferred = <T>() => {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
};

describe("shared script atoms", () => {
  it("keeps character selection, the editor, and character selection synchronized", () => {
    const store = makeStore();
    store.set(appendCharacterRowAtom, "anime");
    const row = store.get(selectedRowAtom)!;
    expect(row.characterId).toBe("anime");
    expect(store.get(selectedRowIndexAtom)).toBe(3);
    expect(store.get(activeCharacterAtom)?.id).toBe("anime");
    expect(store.get(focusRowAtom)).toBe(row.id);
    store.set(editSelectedRowAtom, { text: "追加したセリフ", characterId: "small" });
    expect(store.get(rowsAtom).at(-1)?.text).toBe("追加したセリフ");
    expect(store.get(selectedCharacterAtom)?.id).toBe("small");
    expect(store.get(activeCharacterAtom)?.id).toBe("small");
  });

  it("notifies only the edited row and retains atom identity and take choices when reordering", () => {
    const store = makeStore();
    const [first, second, third] = store.get(rowsAtom);
    store.set(selectRowAtom, first.id);
    const rowAtoms = store.get(rowAtomsAtom);
    const listener = vi.fn();
    const unsubscribe = store.sub(rowAtoms[1], listener);
    store.set(editSelectedRowAtom, { text: "更新", prompt: "小声", takeId: "chosen" });
    expect(listener).not.toHaveBeenCalled();
    expect(store.get(rowAtomsAtom)).toBe(rowAtoms);
    store.set(reorderRowsAtom, first.id, third.id, "after");
    expect(store.get(rowAtomsAtom)[2]).toBe(rowAtoms[0]);
    expect(store.get(selectedRowAtom)).toMatchObject({ id: first.id, text: "更新", prompt: "小声", takeId: "chosen" });
    expect(store.get(rowsAtom)[0]).toBe(second);
    store.set(insertSelectedRowAtom, "duplicate");
    expect(store.get(selectedRowAtom)).toMatchObject({ text: "更新", prompt: "小声" });
    expect(store.get(selectedRowAtom)?.takeId).toBeUndefined();
    unsubscribe();
  });

  it("moves and deletes the selection consistently from any consumer", () => {
    const store = makeStore();
    const [first, second, third] = store.get(rowsAtom);
    store.set(selectRowAtom, second.id);
    store.set(moveSelectedRowAtom, 1);
    expect(store.get(selectedRowIndexAtom)).toBe(2);
    store.set(removeSelectedRowAtom);
    expect(store.get(selectedRowIdAtom)).toBe(third.id);
    store.set(removeSelectedRowAtom);
    store.set(removeSelectedRowAtom);
    expect(store.get(rowsAtom)).toEqual([first]);
    expect(store.get(selectedRowIdAtom)).toBe(first.id);
  });

  it("keeps character editing available at the row limit and preserves takes after character deletion", () => {
    const store = makeStore();
    const rows = Array.from({ length: MAX_SCRIPT_ROWS }, () => newRow("mf"));
    store.set(rowsAtom, rows);
    store.set(appendCharacterRowAtom, "small");
    expect(store.get(rowsAtom)).toBe(rows);
    expect(store.get(activeCharacterAtom)?.id).toBe("small");
    expect(store.get(errorAtom)).toContain("50");
    store.set(rowsAtom, [rows[0]]);
    const take = takeFor(rows[0].id);
    store.set(takesAtom, [take]);
    for (const character of characters) store.set(characterRemovedAtom, character.id);
    expect(store.get(selectedRowAtom)?.characterId).toBe("");
    expect(store.get(takesAtom)).toEqual([take]);
    store.set(appendTakeRowAtom, take);
    expect(store.get(rowsAtom).at(-1)).toMatchObject({ characterId: "", text: take.text });
  });

  it("publishes saved characters without adding lines, and keeps separate stores independent", () => {
    const first = makeStore(),
      second = makeStore();
    first.set(editingCharacterAtom, null);
    first.set(characterSavedAtom, { ...characters[0], id: "new", name: "新規" });
    expect(first.get(activeCharacterAtom)?.id).toBe("new");
    expect(first.get(editingCharacterAtom)).toBeUndefined();
    expect(first.get(rowsAtom)).toHaveLength(3);
    expect(second.get(charactersAtom)).toHaveLength(3);
    first.set(selectRowAtom, first.get(rowsAtom)[2].id);
    expect(second.get(selectedRowIndexAtom)).toBe(0);
  });
});

describe("shared generation atoms", () => {
  it("allows one queue, stops after the in-flight take, and saves to the original row after selection changes", async () => {
    const store = makeStore();
    const [first, second] = store.get(rowsAtom);
    const pending = deferred<Take>();
    vi.mocked(generateTake).mockReturnValueOnce(pending.promise);
    const job = store.set(generateAllRowsAtom);
    await vi.waitFor(() => expect(generateTake).toHaveBeenCalledTimes(1));
    expect(store.get(busyAtom)).toBe(true);
    expect(store.get(runningRowAtom)).toBe(first.id);
    store.set(selectRowAtom, second.id);
    const settings = store.get(selectedRowAtom)?.settings;
    await store.set(generateSelectedRowAtom, true);
    await store.set(generateAllRowsAtom);
    expect(generateTake).toHaveBeenCalledTimes(1);
    expect(store.get(selectedRowAtom)?.settings).toBe(settings);
    store.set(stopGenerationAtom);
    expect(store.get(stoppingAtom)).toBe(true);
    pending.resolve(takeFor(first.id));
    await job;
    expect(store.get(selectedRowIdAtom)).toBe(second.id);
    expect(store.get(rowsAtom)[0].takeId).toBe("take-1");
    expect(store.get(busyAtom)).toBe(false);
    expect(store.get(runningRowAtom)).toBeNull();
    expect(store.get(stoppingAtom)).toBe(false);
    expect(generateTake).toHaveBeenCalledTimes(1);
  });

  it("uses the bundled anime model, snapshots a queue, and continues after a failed row", async () => {
    const store = makeStore();
    const [first, second, third] = store.get(rowsAtom);
    const pending = deferred<Take>();
    vi.mocked(generateTake)
      .mockReturnValueOnce(pending.promise)
      .mockRejectedValueOnce(new Error("生成失敗"))
      .mockResolvedValueOnce(takeFor(third.id, "third"));
    const job = store.set(generateAllRowsAtom);
    await vi.waitFor(() => expect(generateTake).toHaveBeenCalledTimes(1));
    store.set(rowsAtom, (rows) => rows.map((row) => (row.id === third.id ? { ...row, text: "後から変更" } : row)));
    pending.resolve(takeFor(first.id));
    await job;
    const requests = vi.mocked(prepareGeneration).mock.calls[0][0];
    expect(requests.every((input) => !("model" in input.request))).toBe(true);
    expect(requests[2].request.text).toBe("anime");
    expect(store.get(generationErrorsAtom)[second.id]).toBe("生成失敗");
    expect(store.get(takesAtom)).toHaveLength(2);
    expect(store.get(busyAtom)).toBe(false);
  });

  it("validates the entire queue before issuing a request and respects a preview's busy state", async () => {
    const store = makeStore();
    store.set(rowsAtom, (rows) => [...rows, { ...newRow("missing"), text: "未設定" }]);
    await store.set(generateAllRowsAtom);
    expect(generateTake).not.toHaveBeenCalled();
    expect(store.get(errorAtom)).toContain("プリセット");
    store.set(busyAtom, true);
    const rows = store.get(rowsAtom);
    await store.set(generateSelectedRowAtom, true);
    store.set(editSelectedRowAtom, { text: "変更不可" });
    store.set(insertSelectedRowAtom);
    expect(store.get(rowsAtom)).toBe(rows);
    expect(store.get(busyAtom)).toBe(true);
  });

  it("regenerates with a distinct seed while keeping previous takes and per-row take selection", async () => {
    const store = makeStore();
    const [first, second] = store.get(rowsAtom);
    const old = takeFor(first.id, "old", 19);
    store.set(takesAtom, [old]);
    vi.spyOn(crypto, "getRandomValues").mockReturnValueOnce(new Uint32Array([19]));
    vi.mocked(generateTake).mockResolvedValueOnce(takeFor(first.id, "new", 20));
    await store.set(generateSelectedRowAtom, true);
    expect(vi.mocked(prepareGeneration).mock.calls[0][0][0].request.seed).toBe(20);
    expect(store.get(selectedTakeAtom)?.id).toBe("new");
    expect(store.get(selectedRowTakesAtom)).toHaveLength(2);
    store.set(editSelectedRowAtom, { takeId: "old" });
    store.set(selectRowAtom, second.id);
    store.set(selectRowAtom, first.id);
    expect(store.get(selectedTakeAtom)).toBe(old);
  });

  it("keeps audio on deletion failure and falls back to the next take after successful deletion", async () => {
    const store = makeStore();
    const first = store.get(rowsAtom)[0];
    const old = takeFor(first.id, "old"),
      latest = takeFor(first.id, "latest");
    store.set(takesAtom, [latest, old]);
    store.set(editSelectedRowAtom, { takeId: "latest" });
    vi.mocked(removeTake).mockRejectedValueOnce(new Error("削除失敗")).mockResolvedValueOnce(undefined);
    await store.set(deleteTakeAtom, latest.id);
    expect(store.get(selectedTakeAtom)).toBe(latest);
    expect(store.get(deletingTakeAtom)).toBeNull();
    await store.set(deleteTakeAtom, latest.id);
    expect(store.get(selectedTakeAtom)).toBe(old);
  });
});
