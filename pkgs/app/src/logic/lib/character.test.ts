import { expect, it } from "vitest";
import { characterStyle, DEFAULT_CHARACTER_COLOR } from "./character.ts";

it("chooses readable text for character accent colors and falls back for invalid colors", () => {
  expect(characterStyle("#ffffff")).toMatchObject({
    "--character-color": "#ffffff",
    "--character-on-color": "#000000",
  });
  expect(characterStyle("#000000")).toMatchObject({
    "--character-color": "#000000",
    "--character-on-color": "#ffffff",
  });
  expect(characterStyle("invalid")).toMatchObject({ "--character-color": DEFAULT_CHARACTER_COLOR });
});
