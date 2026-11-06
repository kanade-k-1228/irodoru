import type { CSSProperties } from "react";

export const DEFAULT_CHARACTER_COLOR = "#41a2ec";
export const DEFAULT_SAMPLE_TEXT = "こんにちは。よろしくお願いします。";

export const characterStyle = (color = DEFAULT_CHARACTER_COLOR): CSSProperties => {
  const hex = /^#[\da-f]{6}$/i.test(color) ? color : DEFAULT_CHARACTER_COLOR;
  const channels = [1, 3, 5].map((index) => {
    const value = Number.parseInt(hex.slice(index, index + 2), 16) / 255;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  });
  const luminance = channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722;
  return {
    "--character-color": hex,
    "--character-on-color": luminance > 0.179 ? "#000000" : "#ffffff",
  } as CSSProperties;
};
