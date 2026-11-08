import { atom } from "jotai";
import { splitAtom } from "jotai/utils";
import type { Take } from "@/logic/lib/api.ts";
import {
  DEFAULT_PREFERENCES,
  duplicateRow,
  type GenerationSettings,
  insertRow,
  MAX_SCRIPT_ROWS,
  moveRow,
  newRow,
  reorderRow,
  type ScriptRow,
} from "@/logic/lib/script.ts";
import { busyAtom, errorAtom, idleAtom } from "./app.ts";
import { activeCharacterAtom, activeCharacterIdAtom, charactersAtom, editingCharacterAtom } from "./characters.ts";

export const preferencesAtom = atom<GenerationSettings>(DEFAULT_PREFERENCES);
export const advancedSettingsOpenAtom = atom(false);
export const rowsAtom = atom<ScriptRow[]>([]);
export const rowAtomsAtom = splitAtom(rowsAtom, (row) => row.id);
export const rowCountAtom = atom((get) => get(rowsAtom).length);
const selectionAtom = atom<string | null>(null);
export const selectedRowAtom = atom((get) => {
  const rows = get(rowsAtom);
  return rows.find((row) => row.id === get(selectionAtom)) ?? rows[0];
});
export const selectedRowIdAtom = atom((get) => get(selectedRowAtom)?.id);
export const selectedRowIndexAtom = atom((get) => get(rowsAtom).findIndex((row) => row.id === get(selectedRowIdAtom)));
export const selectedCharacterAtom = atom((get) =>
  get(charactersAtom).find((character) => character.id === get(selectedRowAtom)?.characterId),
);
export const focusRowAtom = atom<string | null>(null);
export const canAppendRowAtom = atom((get) => get(idleAtom) && get(rowCountAtom) < MAX_SCRIPT_ROWS);
export const canAddRowAtom = atom((get) => get(canAppendRowAtom) && !!get(charactersAtom).length);

export const selectRowAtom = atom(null, (get, set, id: string, focus = false) => {
  const row = get(rowsAtom).find((row) => row.id === id);
  if (!row) return;
  set(selectionAtom, id);
  set(activeCharacterIdAtom, row.characterId);
  if (focus) set(focusRowAtom, id);
});

export const updateRowAtom = atom(null, (get, set, id: string, update: Partial<ScriptRow>) => {
  set(rowsAtom, (all) => all.map((row) => (row.id === id ? { ...row, ...update } : row)));
  if (id === get(selectedRowIdAtom) && update.characterId !== undefined) {
    set(activeCharacterIdAtom, update.characterId);
  }
});
export const editSelectedRowAtom = atom(null, (get, set, update: Partial<ScriptRow>) => {
  const row = get(selectedRowAtom);
  if (row && !get(busyAtom)) set(updateRowAtom, row.id, update);
});

export const appendCharacterRowAtom = atom(null, (get, set, characterId: string) => {
  if (!get(idleAtom) || !get(charactersAtom).some((character) => character.id === characterId)) return;
  set(activeCharacterIdAtom, characterId);
  if (get(rowCountAtom) >= MAX_SCRIPT_ROWS) {
    set(errorAtom, `台本は${MAX_SCRIPT_ROWS}行までです`);
    return;
  }
  const row = newRow(characterId, get(preferencesAtom));
  set(rowsAtom, (all) => insertRow(all, row));
  set(selectRowAtom, row.id, true);
});

export const insertSelectedRowAtom = atom(null, (get, set, mode: "new" | "duplicate" = "new", afterId?: string) => {
  if (!get(canAddRowAtom)) return;
  const selected = afterId ? get(rowsAtom).find((row) => row.id === afterId) : get(selectedRowAtom);
  if (afterId && !selected) return;
  const character = get(activeCharacterAtom) ?? get(charactersAtom)[0];
  const defaults = get(preferencesAtom);
  const row =
    mode === "duplicate" && selected ? duplicateRow(selected, defaults) : newRow("", selected?.settings ?? defaults);
  row.characterId = get(charactersAtom).some((item) => item.id === selected?.characterId)
    ? selected!.characterId
    : character.id;
  set(rowsAtom, (all) => insertRow(all, row, selected?.id));
  set(selectRowAtom, row.id, true);
});
export const moveSelectedRowAtom = atom(null, (get, set, delta: number) => {
  const row = get(selectedRowAtom);
  if (row && !get(busyAtom)) set(rowsAtom, (all) => moveRow(all, row.id, delta));
});
export const reorderRowsAtom = atom(null, (get, set, id: string, targetId: string, edge: "before" | "after") => {
  if (get(busyAtom)) return;
  set(rowsAtom, (all) => reorderRow(all, id, targetId, edge));
  set(selectRowAtom, id);
});
export const removeSelectedRowAtom = atom(null, (get, set) => {
  const rows = get(rowsAtom);
  const selected = get(selectedRowAtom);
  if (!selected || get(busyAtom) || rows.length <= 1) return;
  const index = get(selectedRowIndexAtom);
  const next = rows[index + 1] ?? rows[index - 1];
  set(
    rowsAtom,
    rows.filter((row) => row.id !== selected.id),
  );
  set(selectRowAtom, next.id, true);
});
export const appendTakeRowAtom = atom(null, (get, set, take: Take) => {
  if (!get(canAppendRowAtom)) return;
  const characterId = get(charactersAtom).some((character) => character.id === take.character_id)
    ? (take.character_id ?? "")
    : "";
  set(rowsAtom, (all) => insertRow(all, { ...newRow(characterId), text: take.text, prompt: take.additional_prompt }));
});
export const characterRemovedAtom = atom(null, (get, set, id: string) => {
  set(charactersAtom, (all) => all.filter((character) => character.id !== id));
  set(rowsAtom, (all) => all.map((row) => (row.characterId === id ? { ...row, characterId: "" } : row)));
  if (get(activeCharacterIdAtom) === id) set(activeCharacterIdAtom, "");
  set(editingCharacterAtom, undefined);
});
