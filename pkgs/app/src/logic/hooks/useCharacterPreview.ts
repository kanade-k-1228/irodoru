import { useAtomValue, useSetAtom } from "jotai";
import { useEffect, useRef, useState } from "react";
import { characterPreview, fetchPreviewAudio, previewCharacter, removePreview } from "@/logic/lib/api.ts";
import { speechRequest } from "@/logic/lib/script.ts";
import { busyAtom } from "@/logic/state/app.ts";
import { defaultSettingsAtom } from "@/logic/state/script.ts";
import type { CharacterPreviewProps } from "@/view/editor/character/CharacterPreview.tsx";

export const useCharacterPreview = ({
  characterId,
  onGenerated,
  text,
  description,
  voice,
  disabled,
  onError,
}: CharacterPreviewProps) => {
  const defaults = useAtomValue(defaultSettingsAtom);
  const setBusy = useSetAtom(busyAtom);
  const [settings, setSettings] = useState(() => ({ ...defaults, numSteps: "", seed: defaults.seed || "1" }));
  const [generating, setGenerating] = useState(false);
  const [sample, setSample] = useState<{ id: string; url: string; settings: string; stored?: boolean }>();
  const request = useRef<AbortController | null>(null);
  const currentSettings = JSON.stringify({ text, description, voice, settings });
  const source = sample?.url;

  useEffect(
    () => () => {
      if (sample) {
        URL.revokeObjectURL(sample.url);
        if (!sample.stored) void removePreview(sample.id).catch(() => {});
      }
    },
    [sample],
  );
  useEffect(
    () => () => {
      if (request.current) {
        request.current.abort();
        request.current = null;
      }
    },
    [],
  );

  useEffect(() => {
    if (!characterId) return;
    let disposed = false;
    void characterPreview(characterId)
      .then(async (id) => {
        if (!id) return;
        const blob = await fetchPreviewAudio(id);
        if (!disposed)
          setSample((current) => current ?? { id, url: URL.createObjectURL(blob), settings: "", stored: true });
      })
      .catch(() => {});
    return () => {
      disposed = true;
    };
  }, [characterId]);

  const generate = async () => {
    if (disabled || request.current || !text.trim()) return;
    const controller = new AbortController();
    request.current = controller;
    setGenerating(true);
    setBusy(true);
    onError(null);
    try {
      const id = await previewCharacter(speechRequest(text, settings), description.trim(), voice || null);
      try {
        const blob = await fetchPreviewAudio(id);
        if (controller.signal.aborted) await removePreview(id);
        else {
          setSample({ id, url: URL.createObjectURL(blob), settings: currentSettings });
          onGenerated(id);
        }
      } catch (error) {
        await removePreview(id).catch(() => {});
        throw error;
      }
    } catch (error) {
      if (!controller.signal.aborted) onError((error as Error).message);
    } finally {
      setBusy(false);
      if (request.current === controller) {
        request.current = null;
        setGenerating(false);
      }
    }
  };

  return { settings, setSettings, generating, sample, source, currentSettings, generate };
};
