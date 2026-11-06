import { useSetAtom } from "jotai";
import { useEffect, useState } from "react";
import { fetchTakeAudio } from "@/logic/lib/api.ts";
import { errorAtom } from "@/logic/state/app.ts";

export function useTakeAudio(id?: string, enabled = true) {
  const [source, setSource] = useState<string>();
  const [loading, setLoading] = useState(false);
  const setError = useSetAtom(errorAtom);
  useEffect(() => {
    setSource(undefined);
    if (!id || !enabled) {
      setLoading(false);
      return;
    }
    setLoading(true);
    const controller = new AbortController();
    let url: string | undefined;
    void fetchTakeAudio(id, controller.signal)
      .then((blob) => {
        if (controller.signal.aborted) return;
        url = URL.createObjectURL(blob);
        setSource(url);
      })
      .catch((error: Error) => {
        if (!controller.signal.aborted) setError(error.message);
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => {
      controller.abort();
      if (url) URL.revokeObjectURL(url);
    };
  }, [id, enabled, setError]);
  return { source, loading };
}
