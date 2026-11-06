import { atom } from "jotai";
import type { Character } from "@/logic/lib/api.ts";

export const charactersAtom = atom<Character[]>([]);
export const activeCharacterIdAtom = atom("");
export const activeCharacterAtom = atom((get) =>
  get(charactersAtom).find((character) => character.id === get(activeCharacterIdAtom)),
);
export const editingCharacterAtom = atom<Character | null | undefined>(undefined);

export const characterSavedAtom = atom(null, (_get, set, character: Character) => {
  set(charactersAtom, (all) =>
    [...all.filter((item) => item.id !== character.id), character].sort((a, b) => a.name.localeCompare(b.name)),
  );
  set(activeCharacterIdAtom, character.id);
  set(editingCharacterAtom, undefined);
});
