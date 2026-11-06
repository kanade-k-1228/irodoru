import { describe, expect, it } from "vitest";
import type { Character } from "./api.ts";
import {
  DEFAULT_PREFERENCES,
  duplicateRow,
  insertRow,
  MAX_SCRIPT_ROWS,
  moveRow,
  newRow,
  takeRequests,
} from "./script.ts";

const character: Character = {
  id: "sakura",
  name: "さくら",
  description: "落ち着いた声",
  voice: null,
  color: "#41a2ec",
  sample_text: "こんにちは。",
};
const defaults = { ...DEFAULT_PREFERENCES, seed: "19" };

describe("script editing", () => {
  it("copies generation settings but only duplicates text and prompts explicitly", () => {
    const original = { ...newRow(character.id, defaults), text: "こんにちは", prompt: "小声で" };
    const next = newRow(character.id, original.settings);
    const copy = duplicateRow(original, defaults);
    expect(next.text).toBe("");
    expect(next.prompt).toBe("");
    expect(copy.text).toBe(original.text);
    expect(copy.prompt).toBe(original.prompt);
    expect(new Set([original.id, next.id, copy.id]).size).toBe(3);
    copy.settings!.seed = "42";
    expect(original.settings?.seed).toBe("19");
    expect(next.settings?.seed).toBe("19");
  });

  it("inserts and moves rows without modifying existing rows or exceeding the limit", () => {
    const first = newRow(character.id);
    const second = newRow(character.id);
    const added = newRow(character.id);
    const rows = [first, second];
    expect(insertRow(rows, added, first.id)).toEqual([first, added, second]);
    expect(insertRow(rows, added, "deleted-row")).toEqual([first, second, added]);
    expect(moveRow(rows, second.id, -1)).toEqual([second, first]);
    expect(moveRow(rows, first.id, -1)).toBe(rows);
    expect(moveRow(rows, "deleted-row", 1)).toBe(rows);
    expect(rows).toEqual([first, second]);
    const full = Array.from({ length: MAX_SCRIPT_ROWS }, () => newRow(character.id));
    expect(insertRow(full, added)).toBe(full);
  });
});

describe("generation requests", () => {
  it("loads older drafts, skips blank lines, and keeps per-line settings separate", () => {
    const oldDraft = { id: "old", text: "こんにちは", characterId: character.id };
    const custom = {
      ...newRow(character.id, { ...defaults, seed: "0", numSteps: "40" }),
      text: "次の行",
      prompt: "  小声で  ",
    };
    const queue = takeRequests([newRow(""), oldDraft, custom], [character], defaults);
    expect(queue).toHaveLength(2);
    expect(queue[0]).toMatchObject({ row_id: "old", additional_prompt: null, request: { seed: 19 } });
    expect(queue[0].request.num_steps).toBeNull();
    expect(queue[0].request).not.toHaveProperty("model");
    expect(queue[1]).toMatchObject({ additional_prompt: "小声で", request: { seed: 0, num_steps: 40 } });
    custom.settings!.seed = "123";
    expect(queue[1].request.seed).toBe(0);
  });

  it("rejects the entire queue when a character is missing", () => {
    const valid = { ...newRow(character.id), text: "最初" };
    const invalid = { ...newRow("deleted"), text: "次" };
    expect(() => takeRequests([valid, invalid], [character], defaults)).toThrow("プリセット");
  });

  it.each(["-1", "1.5", "9007199254740992", "invalid"])("rejects unsafe seed %s", (seed) => {
    const row = { ...newRow(character.id, { ...defaults, seed }), text: "こんにちは" };
    expect(() => takeRequests([row], [character], defaults)).toThrow("シード");
  });

  it.each(["0", "201", "2.5", "invalid"])("rejects invalid step count %s", (numSteps) => {
    const row = { ...newRow(character.id, { ...defaults, numSteps }), text: "こんにちは" };
    expect(() => takeRequests([row], [character], defaults)).toThrow("ステップ数");
  });
});
