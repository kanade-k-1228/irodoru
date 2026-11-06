import { useSetAtom } from "jotai";
import { useEffect } from "react";
import { inferenceStatus } from "@/logic/lib/api.ts";
import type { InferenceStatus } from "@/logic/lib/desktop.d.ts";
import { inferenceStatusAtom } from "@/logic/state/app.ts";

export function useInferenceStatus() {
  const setStatus = useSetAtom(inferenceStatusAtom);
  useEffect(() => {
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      const next = await inferenceStatus().catch(
        (error: Error): InferenceStatus => ({ state: "error", error: error.message }),
      );
      if (disposed) return;
      setStatus(next);
      timer = setTimeout(() => void poll(), 1000);
    };
    void poll();
    return () => {
      disposed = true;
      clearTimeout(timer);
    };
  }, [setStatus]);
}
