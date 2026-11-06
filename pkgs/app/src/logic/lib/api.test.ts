import { invoke } from "@tauri-apps/api/core";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  exportTake,
  fetchPreviewAudio,
  fetchTakeAudio,
  fetchTakes,
  inferenceStatus,
  prepareGeneration,
  saveCharacter,
  uploadVoice,
} from "./api.ts";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), convertFileSrc: (path: string) => `asset:${path}` }));
beforeEach(() => {
  vi.mocked(invoke).mockReset();
});
it("normalizes Rust command errors", async () => {
  vi.mocked(invoke).mockRejectedValue("disk full");
  const result = await fetchTakes().catch((error: unknown) => error);
  expect(result).toBeInstanceOf(Error);
  expect((result as Error).message).toBe("disk full");
});
it("writes characters locally and copies imported WAV bytes", async () => {
  const input = { name: "さくら", color: null, sample_text: null, description: "低め", voice: null };
  vi.mocked(invoke).mockResolvedValue({ id: "id", ...input });
  await saveCharacter(input, "id");
  expect(invoke).toHaveBeenCalledWith("save_character", { input, id: "id", preview: null });
  await uploadVoice("reference", new Blob([new Uint8Array([82, 73, 70, 70])]));
  expect(invoke).toHaveBeenLastCalledWith("upload_voice", { name: "reference", wav: [82, 73, 70, 70] });
});
it("prepares the entire queue for embedded inference before generation", async () => {
  vi.mocked(invoke).mockResolvedValue("batch");
  expect(await prepareGeneration([])).toBe("batch");
  expect(invoke).toHaveBeenCalledWith("prepare_generation", { requests: [] });
});
it("checks the embedded model state", async () => {
  vi.mocked(invoke).mockResolvedValue({ state: "ready", error: null });
  expect(await inferenceStatus()).toEqual({ state: "ready", error: null });
  expect(invoke).toHaveBeenCalledWith("inference_status", undefined);
});
it("delegates export to a native dialog", async () => {
  await exportTake("id");
  expect(invoke).toHaveBeenLastCalledWith("export_take", { id: "id" });
});

afterEach(() => vi.unstubAllGlobals());
it("reads scoped audio bytes with cancellation for WebKit playback", async () => {
  vi.mocked(invoke).mockResolvedValue("/data/takes/id/audio.wav");
  const fetch = vi.fn().mockResolvedValue(new Response(new Uint8Array([82, 73, 70, 70])));
  vi.stubGlobal("fetch", fetch);
  const signal = new AbortController().signal;
  const wav = await fetchTakeAudio("id", signal);
  expect(wav.type).toBe("audio/wav");
  expect(await wav.text()).toBe("RIFF");
  expect(fetch).toHaveBeenCalledWith("asset:/data/takes/id/audio.wav", { signal });
  expect(invoke).toHaveBeenCalledWith("take_audio_path", { id: "id" });
});
it("reports preview audio read failures", async () => {
  vi.mocked(invoke).mockResolvedValue("/data/previews/id.wav");
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response(null, { status: 404 })));
  await expect(fetchPreviewAudio("id")).rejects.toThrow("サンプル音声を読み込めませんでした");
  expect(invoke).toHaveBeenCalledWith("preview_audio_path", { id: "id" });
});

it("waits for editor writes before preparing generation and recovers after a failed save", async () => {
  const { saveEditor } = await import("./api.ts");
  const { DEFAULT_PREFERENCES } = await import("./script.ts");
  const editor = { rows: [], preferences: DEFAULT_PREFERENCES, activeCharacter: "" };
  let finish!: () => void;
  vi.mocked(invoke).mockImplementationOnce(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  const saving = saveEditor(editor);
  const preparing = prepareGeneration([]);
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));
  expect(invoke).toHaveBeenLastCalledWith("save_editor", { editor });
  finish();
  await saving;
  await preparing;
  expect(invoke).toHaveBeenLastCalledWith("prepare_generation", { requests: [] });
  vi.mocked(invoke).mockRejectedValueOnce("disk full");
  await expect(saveEditor(editor)).rejects.toThrow("disk full");
  vi.mocked(invoke).mockResolvedValueOnce(undefined);
  await expect(saveEditor(editor)).resolves.toBeUndefined();
});
