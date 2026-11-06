import { useAtomValue, useSetAtom } from "jotai";
import { useEffect, useMemo, useRef, useState } from "react";
import { type Character, fetchVoices, removeCharacter, saveCharacter } from "@/logic/lib/api.ts";
import { DEFAULT_CHARACTER_COLOR, DEFAULT_SAMPLE_TEXT } from "@/logic/lib/character.ts";
import { busyAtom, idleAtom } from "@/logic/state/app.ts";
import { characterSavedAtom, editingCharacterAtom } from "@/logic/state/characters.ts";
import { voicesAtom } from "@/logic/state/library.ts";
import { characterRemovedAtom } from "@/logic/state/script.ts";

export const useCharacterEditor = (character: Character | null) => {
  const setVoices = useSetAtom(voicesAtom);
  const setEditing = useSetAtom(editingCharacterAtom);
  const onClose = () => setEditing(undefined);
  const onSaved = useSetAtom(characterSavedAtom);
  const onDeleted = useSetAtom(characterRemovedAtom);
  const nameInput = useRef<HTMLInputElement>(null);
  const initial = useMemo(
    () => ({
      name: character?.name ?? "",
      description: character?.description ?? "",
      voice: character?.voice ?? "",
      color: character?.color ?? DEFAULT_CHARACTER_COLOR,
      sample_text: character?.sample_text ?? DEFAULT_SAMPLE_TEXT,
      icon: character?.icon ?? "",
    }),
    [character],
  );
  const [previewId, setPreviewId] = useState<string>();
  const [draft, setDraft] = useState(initial);
  const [saving, setSaving] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const busy = useAtomValue(busyAtom);
  const idle = useAtomValue(idleAtom);
  const dirty = !!previewId || JSON.stringify(draft) !== JSON.stringify(initial);
  const disabled = saving || uploading || !idle;
  const change = (update: Partial<typeof draft>) => setDraft((current) => ({ ...current, ...update }));

  useEffect(() => {
    nameInput.current?.focus();
  }, []);

  const close = () => {
    if (saving || uploading || busy) return;
    if (dirty && !confirm("未保存の変更を破棄しますか？")) return;
    onClose();
  };
  const save = async () => {
    if (disabled || !draft.name.trim()) return;
    setSaving(true);
    setError(null);
    try {
      const saved = await saveCharacter(
        { ...draft, name: draft.name.trim(), voice: draft.voice || null },
        character?.id,
        previewId,
      );
      const voices = await fetchVoices().catch(() => null);
      if (voices) setVoices(voices);
      onSaved(saved);
    } catch (error) {
      setError((error as Error).message);
    } finally {
      setSaving(false);
    }
  };
  const remove = async () => {
    if (disabled || !character || !confirm(`「${character.name}」を削除しますか？生成済みの音声は残ります。`)) return;
    setSaving(true);
    setError(null);
    try {
      await removeCharacter(character.id);
      onDeleted(character.id);
    } catch (error) {
      setError((error as Error).message);
    } finally {
      setSaving(false);
    }
  };

  return {
    characterId: character?.id,
    setPreviewId,
    nameInput,
    draft,
    saving,
    error,
    dirty,
    disabled,
    change,
    setUploading,
    setError,
    close,
    save,
    remove,
  };
};
