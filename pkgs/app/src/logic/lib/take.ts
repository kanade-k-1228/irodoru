const MODEL_LABELS: Record<string, string> = { mf: "MF", small: "Small", anime: "Anime" };

export const modelLabel = (model: string) => MODEL_LABELS[model] ?? model;
