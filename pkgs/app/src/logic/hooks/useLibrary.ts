import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useStore } from "jotai";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  chooseProject,
  fetchCharacters,
  fetchTakes,
  fetchVoices,
  flushProject,
  isMobile,
  loadEditor,
  projectStatus,
  saveEditor,
} from "@/logic/lib/api.ts";
import type { ProjectStatus } from "@/logic/lib/desktop.d.ts";
import { busyAtom, errorAtom, readyAtom } from "@/logic/state/app.ts";
import { generationErrorsAtom, takesAtom } from "@/logic/state/audio.ts";
import { activeCharacterIdAtom, charactersAtom, editingCharacterAtom } from "@/logic/state/characters.ts";
import { voicesAtom } from "@/logic/state/library.ts";
import { preferencesAtom, rowsAtom, selectRowAtom } from "@/logic/state/script.ts";

export function useLibrary() {
  const store = useStore();
  const [status, setStatus] = useState<ProjectStatus>({
    id: null,
    name: null,
    path: null,
    state: "closed",
    error: null,
  });
  const [loading, setLoading] = useState(true);
  const session = useRef<string | null>(null);
  const transitioning = useRef(false);
  const pending = useRef<Promise<void>>(Promise.resolve());
  const error = useCallback((cause: unknown) => store.set(errorAtom, (cause as Error).message), [store]);
  const updateStatus = useCallback(
    (next: ProjectStatus) => {
      if (next.id !== session.current) return;
      setStatus(next);
      if (next.error) store.set(errorAtom, `保存できませんでした: ${next.error}`);
    },
    [store],
  );
  const saveEditorNow = useCallback(() => {
    if (!session.current) return pending.current;
    const editor = {
      rows: store.get(rowsAtom),
      preferences: store.get(preferencesAtom),
      activeCharacter: store.get(activeCharacterIdAtom),
    };
    setStatus((current) => ({ ...current, state: "saving", error: null }));
    const write = saveEditor(editor);
    pending.current = write;
    void write.catch(error);
    return write;
  }, [store, error]);
  const save = useCallback(async () => {
    await saveEditorNow();
    await flushProject();
    updateStatus(await projectStatus());
  }, [saveEditorNow, updateStatus]);
  const load = useCallback(async () => {
    const current = await projectStatus();
    setStatus(current);
    if (!current.id) {
      return;
    }
    const [editor, voices, characters, takes] = await Promise.all([
      loadEditor(),
      fetchVoices(),
      fetchCharacters(),
      fetchTakes(),
    ]);
    session.current = current.id;
    store.set(editingCharacterAtom, undefined);
    store.set(generationErrorsAtom, {});
    store.set(preferencesAtom, editor.preferences);
    store.set(rowsAtom, editor.rows);
    store.set(activeCharacterIdAtom, editor.activeCharacter);
    store.set(voicesAtom, voices);
    store.set(charactersAtom, characters);
    store.set(takesAtom, takes);
    if (editor.rows[0]) store.set(selectRowAtom, editor.rows[0].id);
    setStatus(await projectStatus());
    store.set(readyAtom, true);
  }, [store]);
  const choose = useCallback(
    async (create: boolean) => {
      if (transitioning.current || store.get(busyAtom) || store.get(editingCharacterAtom) !== undefined) return;
      transitioning.current = true;
      setLoading(true);
      store.set(readyAtom, false);
      store.set(errorAtom, null);
      try {
        await saveEditorNow();
        await pending.current;
        if (await chooseProject(create)) {
          session.current = null;
          await load();
        }
      } catch (cause) {
        error(cause);
      } finally {
        store.set(readyAtom, !!session.current);
        setLoading(false);
        transitioning.current = false;
      }
    },
    [store, saveEditorNow, load, error],
  );
  useEffect(() => {
    let disposed = false;
    const cleanup: (() => void)[] = [];
    const track = (unlisten: () => void) => (disposed ? unlisten() : cleanup.push(unlisten));
    const changed = () => {
      if (!store.get(readyAtom) || transitioning.current) return;
      const ids = new Set(store.get(rowsAtom).map((row) => row.id));
      store.set(takesAtom, (takes) => takes.filter((take) => ids.has(take.row_id)));
      void saveEditorNow().catch(() => {});
    };
    void (async () => {
      track(
        await listen<ProjectStatus>("project-status", (event) => {
          if (!disposed) updateStatus(event.payload);
        }),
      );
      if (disposed) return;
      await load();
      if (disposed) return;
      for (const atom of [rowsAtom, preferencesAtom, activeCharacterIdAtom]) track(store.sub(atom, changed));
      if (!(await isMobile())) {
        track(
          await getCurrentWindow().onCloseRequested(async (event) => {
            event.preventDefault();
            if (transitioning.current || store.get(busyAtom)) {
              store.set(errorAtom, "処理の完了後に閉じてください。");
              return;
            }
            if (store.get(editingCharacterAtom) !== undefined) {
              store.set(errorAtom, "キャラクタの編集を保存またはキャンセルしてから閉じてください。");
              return;
            }
            transitioning.current = true;
            store.set(readyAtom, false);
            try {
              await save();
              await getCurrentWindow().destroy();
            } catch (cause) {
              error(cause);
            } finally {
              transitioning.current = false;
              store.set(readyAtom, !!session.current);
            }
          }),
        );
      }
    })()
      .catch(error)
      .finally(() => {
        if (!disposed) setLoading(false);
      });
    return () => {
      disposed = true;
      for (const fn of cleanup) fn();
    };
  }, [store, saveEditorNow, save, load, updateStatus, error]);
  return {
    status,
    loading,
    save: () => void save().catch(error),
    create: () => void choose(true),
    open: () => void choose(false),
  };
}
