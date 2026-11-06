import { atom } from "jotai";
import { atomWithStorage } from "jotai/utils";
import type { InferenceStatus } from "@/logic/lib/desktop.d.ts";

export const themeAtom = atomWithStorage<"dark" | "light">("irodoru.theme", "dark", undefined, { getOnInit: true });
export const readyAtom = atom(false);
export const busyAtom = atom(false);
export const idleAtom = atom((get) => !get(busyAtom) && get(readyAtom));
export const errorAtom = atom<string | null>(null);

export const inferenceStatusAtom = atom<InferenceStatus>({ state: "loading" });
export const canGenerateAtom = atom((get) => get(idleAtom) && get(inferenceStatusAtom).state === "ready");
