import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type {
  Character,
  CharacterInput,
  EditorState,
  InferenceStatus,
  ProjectStatus,
  SynthesisInput,
  Take,
  TakeRequest,
} from "./desktop.d.ts";

export type { Character, CharacterInput, Take, TakeRequest } from "./desktop.d.ts";

async function command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(name, args);
  } catch (error) {
    throw error instanceof Error ? error : new Error(String(error));
  }
}
export const fetchCharacters = () => command<Character[]>("fetch_characters");
export const isMobile = () => command<boolean>("is_mobile");
export const newWindow = () => command<void>("new_window");
export const inferenceStatus = () => command<InferenceStatus>("inference_status");
export const saveCharacter = (input: CharacterInput, id?: string, preview?: string) =>
  command<Character>("save_character", { input, id: id ?? null, preview: preview ?? null });
export const removeCharacter = (id: string) => command<void>("remove_character", { id });
export const fetchVoices = () => command<string[]>("fetch_voices");
export async function uploadVoice(name: string, wav: Blob): Promise<void> {
  await command<void>("upload_voice", { name, wav: Array.from(new Uint8Array(await wav.arrayBuffer())) });
}
export const fetchTakes = () => command<Take[]>("fetch_takes");
export const removeTake = (id: string) => command<void>("remove_take", { id });
export const prepareGeneration = async (requests: TakeRequest[]) => {
  await pendingEditor;
  return command<string>("prepare_generation", { requests });
};
export const generateTake = (batch: string, index: number) => command<Take>("generate_take", { batch, index });
export const finishGeneration = (batch: string) => command<void>("finish_generation", { batch });
export const loadEditor = () => command<EditorState>("load_editor");
let pendingEditor: Promise<void> = Promise.resolve();
export const saveEditor = (editor: EditorState) => {
  pendingEditor = pendingEditor.catch(() => {}).then(() => command<void>("save_editor", { editor }));
  return pendingEditor;
};
export const projectStatus = () => command<ProjectStatus>("project_status");
export const chooseProject = (create: boolean) => command<boolean>("choose_project", { create });
export const flushProject = () => command<void>("flush_project");
export const exportTake = (id: string) => command<void>("export_take", { id });
async function fetchWav(path: string, error: string, signal?: AbortSignal): Promise<Blob> {
  const response = await fetch(convertFileSrc(path), { signal });
  if (!response.ok) throw new Error(error);
  return new Blob([await response.arrayBuffer()], { type: "audio/wav" });
}
export const fetchTakeAudio = async (id: string, signal?: AbortSignal) =>
  fetchWav(await command<string>("take_audio_path", { id }), "音声を読み込めませんでした", signal);

export const previewCharacter = (request: SynthesisInput, description: string, voice: string | null) =>
  command<string>("preview_character", { request, description, voice });
export const removePreview = (id: string) => command<void>("remove_preview", { id });
export const exportPreview = (id: string) => command<void>("export_preview", { id });
export const fetchPreviewAudio = async (id: string) =>
  fetchWav(await command<string>("preview_audio_path", { id }), "サンプル音声を読み込めませんでした");

export const characterPreview = (id: string) => command<string | null>("character_preview", { id });
