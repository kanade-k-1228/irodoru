import { useAtomValue, useSetAtom } from "jotai";
import { useEffect, useMemo, useRef, useState } from "react";
import { type Character, removeCharacter, saveCharacter } from "@/logic/lib/api.ts";
import { DEFAULT_CHARACTER_COLOR, DEFAULT_SAMPLE_TEXT } from "@/logic/lib/character.ts";
import { busyAtom, idleAtom } from "@/logic/state/app.ts";
import { characterSavedAtom, editingCharacterAtom } from "@/logic/state/characters.ts";
import { characterRemovedAtom } from "@/logic/state/script.ts";

export const useCharacterEditor = (character: Character | null) => {
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
  const [draft, setDraft] = useState(initial);
  const [saving, setSaving] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const busy = useAtomValue(busyAtom);
  const idle = useAtomValue(idleAtom);
  const dirty = JSON.stringify(draft) !== JSON.stringify(initial);
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
      onSaved(await saveCharacter({ ...draft, name: draft.name.trim(), voice: draft.voice || null }, character?.id));
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
