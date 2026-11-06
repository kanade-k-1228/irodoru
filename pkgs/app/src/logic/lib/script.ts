import type { Character, TakeRequest } from "./api.ts";
import type { GenerationSettings, ScriptRow, SynthesisInput } from "./desktop.d.ts";

export type { GenerationSettings, ScriptRow } from "./desktop.d.ts";

export const MAX_SCRIPT_ROWS = 50;

export const DEFAULT_PREFERENCES: GenerationSettings = {
  seed: "",
  numSteps: "",
  cfgText: 3,
  cfgSpeaker: 5,
  cfgCaption: 3,
  durationScale: 1,
};

export const newRow = (characterId: string, settings?: GenerationSettings): ScriptRow => {
  return {
    id: crypto.randomUUID(),
    text: "",
    characterId,
    prompt: "",
    settings: settings && { ...settings },
  };
};

export const duplicateRow = (source: ScriptRow, defaults: GenerationSettings): ScriptRow => {
  return {
    ...newRow(source.characterId, source.settings ?? defaults),
    text: source.text,
    prompt: source.prompt ?? "",
  };
};

export const insertRow = (rows: ScriptRow[], row: ScriptRow, afterId?: string): ScriptRow[] => {
  if (rows.length >= MAX_SCRIPT_ROWS) return rows;
  const after = rows.findIndex((item) => item.id === afterId);
  const index = after < 0 ? rows.length : after + 1;
  return [...rows.slice(0, index), row, ...rows.slice(index)];
};

export const moveRow = (rows: ScriptRow[], id: string, delta: number): ScriptRow[] => {
  const index = rows.findIndex((row) => row.id === id);
  const next = index + delta;
  if (index < 0 || next < 0 || next >= rows.length) return rows;
  const reordered = [...rows];
  [reordered[index], reordered[next]] = [reordered[next], reordered[index]];
  return reordered;
};

export const reorderRow = (rows: ScriptRow[], id: string, targetId: string, edge: "before" | "after"): ScriptRow[] => {
  const row = rows.find((row) => row.id === id);
  if (!row || id === targetId || !rows.some((row) => row.id === targetId)) return rows;
  const remaining = rows.filter((row) => row.id !== id);
  const index = remaining.findIndex((row) => row.id === targetId) + (edge === "after" ? 1 : 0);
  if (rows.indexOf(row) === index) return rows;
  return [...remaining.slice(0, index), row, ...remaining.slice(index)];
};

const optionalNumber = (value: string) => (value.trim() === "" ? undefined : Number(value));

export const speechRequest = (text: string, settings: GenerationSettings): SynthesisInput => {
  const seed = optionalNumber(settings.seed);
  const steps = optionalNumber(settings.numSteps);
  if (seed !== undefined && (!Number.isSafeInteger(seed) || seed < 0)) {
    throw new Error("シードには0以上の安全な整数を指定してください");
  }
  if (steps !== undefined && (!Number.isInteger(steps) || steps < 1 || steps > 200)) {
    throw new Error("ステップ数は1〜200で指定してください");
  }
  return {
    text,
    seed: seed ?? null,
    num_steps: steps ?? null,
    duration_scale: settings.durationScale,
    cfg_scale_text: settings.cfgText,
    cfg_scale_speaker: settings.cfgSpeaker,
    cfg_scale_caption: settings.cfgCaption,
  };
};

export const takeRequests = (
  rows: ScriptRow[],
  characters: Character[],
  defaults: GenerationSettings,
): TakeRequest[] => {
  return rows
    .filter((row) => row.text.trim())
    .map((row) => {
      const character = characters.find((character) => character.id === row.characterId);
      if (!character) {
        throw new Error("各セリフのプリセットを選択してください");
      }
      return {
        row_id: row.id,
        character_id: row.characterId,
        additional_prompt: row.prompt?.trim() || null,
        request: speechRequest(row.text, row.settings ?? defaults),
      };
    });
};
