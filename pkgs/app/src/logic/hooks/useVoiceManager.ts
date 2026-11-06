import { useAtom } from "jotai";
import { useRef, useState } from "react";
import { fetchVoices, uploadVoice } from "@/logic/lib/api.ts";
import { toWavBlob } from "@/logic/lib/wav.ts";
import { voicesAtom } from "@/logic/state/library.ts";
import type { VoiceManagerProps } from "@/view/editor/character/VoiceManager.tsx";

export const useVoiceManager = ({ name, onChange, onBusy, onError }: VoiceManagerProps) => {
  const [voices, setVoices] = useAtom(voicesAtom);
  const [uploading, setUploading] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);
  const register = async (file: File) => {
    setUploading(true);
    onBusy(true);
    onError(null);
    try {
      const { wav, seconds } = await toWavBlob(file);
      if (seconds < 1) throw new Error("1秒以上の音声が必要です");
      const prefix = (name || file.name.replace(/\.[^.]+$/, "")).replace(/[\p{C}/\\:*?"<>|.]/gu, "_").slice(0, 32);
      const voice = `${prefix || "reference"}-${crypto.randomUUID().slice(0, 8)}`;
      await uploadVoice(voice, wav);
      setVoices(await fetchVoices());
      onChange(voice);
    } catch (e) {
      onError((e as Error).message);
    } finally {
      setUploading(false);
      onBusy(false);
      if (fileRef.current) fileRef.current.value = "";
    }
  };
  return { voices, uploading, fileRef, register };
};
