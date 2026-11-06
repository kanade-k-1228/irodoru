import { getCurrentWindow } from "@tauri-apps/api/window";
import { useStore } from "jotai";
import { useCallback, useEffect, useRef } from "react";
import { fetchCharacters, fetchTakes, fetchVoices, isMobile, loadEditor, saveEditor } from "@/logic/lib/api.ts";
import { busyAtom, errorAtom, readyAtom } from "@/logic/state/app.ts";
import { takesAtom } from "@/logic/state/audio.ts";
import { activeCharacterIdAtom, charactersAtom } from "@/logic/state/characters.ts";
import { voicesAtom } from "@/logic/state/library.ts";
import { preferencesAtom, rowsAtom } from "@/logic/state/script.ts";

export function useLibrary() {
  const store = useStore();
  const saveRef = useRef<(() => void) | null>(null);
  const saveNow = useCallback(() => saveRef.current?.(), []);
  useEffect(() => {
    let disposed = false;
    const cleanup: (() => void)[] = [];
    let pending = Promise.resolve();
    let saveError: Error | null = null;
    const save = () => {
      const editor = {
        rows: store.get(rowsAtom),
        preferences: store.get(preferencesAtom),
        activeCharacter: store.get(activeCharacterIdAtom),
      };
      pending = pending.then(async () => {
        try {
          await saveEditor(editor);
          saveError = null;
        } catch (error) {
          saveError = error as Error;
          store.set(errorAtom, `保存できませんでした: ${saveError.message}`);
        }
      });
    };
    void Promise.all([loadEditor(), fetchVoices(), fetchCharacters(), fetchTakes(), isMobile()])
      .then(async ([editor, voices, characters, takes, mobile]) => {
        if (disposed) return;
        store.set(preferencesAtom, editor.preferences);
        store.set(rowsAtom, editor.rows);
        store.set(activeCharacterIdAtom, editor.activeCharacter);
        store.set(voicesAtom, voices);
        store.set(charactersAtom, characters);
        store.set(takesAtom, takes);
        for (const atom of [rowsAtom, preferencesAtom, activeCharacterIdAtom]) cleanup.push(store.sub(atom, save));
        saveRef.current = save;
        cleanup.push(() => {
          saveRef.current = null;
        });
        if (!mobile) {
          const unlisten = await getCurrentWindow().onCloseRequested(async (event) => {
            event.preventDefault();
            if (store.get(busyAtom)) {
              store.set(errorAtom, "生成中です。停止して生成の完了後に閉じてください。");
              return;
            }
            save();
            await pending;
            if (!saveError)
              await getCurrentWindow()
                .destroy()
                .catch((error: unknown) => store.set(errorAtom, String(error)));
          });
          if (disposed) {
            unlisten();
            return;
          }
          cleanup.push(unlisten);
        }
        store.set(readyAtom, true);
      })
      .catch((error: Error) => {
        if (!disposed) store.set(errorAtom, error.message);
      });
    return () => {
      disposed = true;
      for (const fn of cleanup) fn();
    };
  }, [store]);
  return saveNow;
}
