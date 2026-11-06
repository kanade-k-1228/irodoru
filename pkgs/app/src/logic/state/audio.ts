import { atom } from "jotai";
import { removeTake, type Take, type TakeRequest } from "@/logic/lib/api.ts";
import { generateQueue } from "@/logic/lib/generation.ts";
import { type ScriptRow, takeRequests } from "@/logic/lib/script.ts";
import { busyAtom, errorAtom, idleAtom } from "./app.ts";
import { charactersAtom } from "./characters.ts";
import { defaultSettingsAtom, rowsAtom, selectedRowAtom, selectedRowIdAtom, updateRowAtom } from "./script.ts";

export const takesAtom = atom<Take[]>([]);
export const selectedRowTakesAtom = atom((get) =>
  get(takesAtom).filter((take) => take.row_id === get(selectedRowIdAtom)),
);
export const selectedTakeAtom = atom((get) => {
  const takes = get(selectedRowTakesAtom);
  return takes.find((take) => take.id === get(selectedRowAtom)?.takeId) ?? takes[0];
});
export const runningRowAtom = atom<string | null>(null);
export const stoppingAtom = atom(false);
export const generationErrorsAtom = atom<Record<string, string>>({});

export const generateRowsAtom = atom(null, async (get, set, rows: ScriptRow[]) => {
  if (!get(idleAtom)) return;
  let queue: TakeRequest[];
  try {
    queue = takeRequests(rows, get(charactersAtom), get(defaultSettingsAtom));
  } catch (error) {
    set(errorAtom, (error as Error).message);
    return;
  }
  if (!queue.length) return;
  set(busyAtom, true);
  set(stoppingAtom, false);
  set(errorAtom, null);
  try {
    await generateQueue(queue, {
      shouldStop: () => get(stoppingAtom),
      onStart: (id) => {
        set(runningRowAtom, id);
        set(generationErrorsAtom, (all) => ({ ...all, [id]: "" }));
      },
      onTake: (take) => {
        set(takesAtom, (all) => [take, ...all]);
        set(updateRowAtom, take.row_id, { takeId: take.id });
      },
      onError: (id, error) => set(generationErrorsAtom, (all) => ({ ...all, [id]: error.message })),
    });
  } catch (error) {
    set(errorAtom, (error as Error).message);
    for (const request of queue)
      set(generationErrorsAtom, (all) => ({ ...all, [request.row_id]: (error as Error).message }));
  } finally {
    set(busyAtom, false);
    set(runningRowAtom, null);
    set(stoppingAtom, false);
  }
});
export const generateAllRowsAtom = atom(null, (get, set) => set(generateRowsAtom, get(rowsAtom)));
export const generateSelectedRowAtom = atom(null, (get, set, random = false) => {
  const row = get(selectedRowAtom);
  if (!row || !get(idleAtom)) return;
  if (!random) return set(generateRowsAtom, [row]);
  const previous = get(selectedTakeAtom);
  let seed = crypto.getRandomValues(new Uint32Array(1))[0];
  if (String(seed) === previous?.seed || String(seed) === row.settings?.seed) seed = (seed + 1) >>> 0;
  const settings = { ...(row.settings ?? get(defaultSettingsAtom)), seed: String(seed) };
  set(updateRowAtom, row.id, { settings });
  return set(generateRowsAtom, [{ ...row, settings }]);
});
export const stopGenerationAtom = atom(null, (get, set) => {
  if (get(runningRowAtom)) set(stoppingAtom, true);
});

export const historyFilterAtom = atom("");
export const takeNumbersAtom = atom((get) => {
  const numbers = new Map<string, number>();
  const counters = new Map<string, number>();
  for (const take of [...get(takesAtom)].reverse()) {
    const n = (counters.get(take.row_id) ?? 0) + 1;
    counters.set(take.row_id, n);
    numbers.set(take.id, n);
  }
  return numbers;
});
export const deletingTakeAtom = atom<string | null>(null);
export const deleteTakeAtom = atom(null, async (get, set, id: string) => {
  if (get(deletingTakeAtom)) return;
  set(deletingTakeAtom, id);
  try {
    await removeTake(id);
    set(takesAtom, (all) => all.filter((take) => take.id !== id));
  } catch (error) {
    set(errorAtom, (error as Error).message);
  } finally {
    set(deletingTakeAtom, null);
  }
});
