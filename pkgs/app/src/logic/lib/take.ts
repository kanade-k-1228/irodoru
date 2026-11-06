const MODEL_LABELS: Record<string, string> = { mf: "MF", small: "Small", anime: "Anime" };

export const modelLabel = (model: string) => MODEL_LABELS[model] ?? model;
export const takeLabel = (number?: number) => `TAKE ${String(number).padStart(2, "0")}`;
