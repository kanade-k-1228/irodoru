import { describe, expect, it } from "vitest";
import { downmix, encodeWav } from "./wav.ts";

describe("encodeWav", () => {
  it("writes a valid 16-bit mono PCM header and clamps samples", () => {
    const bytes = encodeWav(new Float32Array([0, 1, -1, 2]), 48000);
    const view = new DataView(bytes.buffer);
    const text = (o: number) => String.fromCharCode(...bytes.slice(o, o + 4));
    expect(text(0)).toBe("RIFF");
    expect(text(8)).toBe("WAVE");
    expect(view.getUint32(24, true)).toBe(48000);
    expect(view.getUint16(34, true)).toBe(16);
    expect(view.getUint32(40, true)).toBe(8);
    expect([0, 1, 2, 3].map((i) => view.getInt16(44 + i * 2, true))).toEqual([0, 32767, -32767, 32767]);
  });
});

describe("downmix", () => {
  it("averages channels", () => {
    expect(Array.from(downmix([new Float32Array([1, 0]), new Float32Array([0, 1])]))).toEqual([0.5, 0.5]);
  });
});
