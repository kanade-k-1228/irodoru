import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow, type Window } from "@tauri-apps/api/window";
import { useSetAtom } from "jotai";
import { useCallback, useEffect, useState } from "react";
import { isMobile } from "@/logic/lib/api.ts";
import { errorAtom } from "@/logic/state/app.ts";

export function useWindowControls() {
  const [appWindow, setAppWindow] = useState<Window | null>(null);
  const [maximized, setMaximized] = useState(false);
  const setError = useSetAtom(errorAtom);
  const reportError = useCallback(
    (cause: unknown) =>
      setError(`ウィンドウを操作できませんでした: ${cause instanceof Error ? cause.message : String(cause)}`),
    [setError],
  );

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void (async () => {
      if ((await isMobile()) || disposed) return;
      const current = getCurrentWindow();
      setAppWindow(current);
      const update = async () => {
        const value = await current.isMaximized();
        if (!disposed) setMaximized(value);
      };
      unlisten = await current.onResized(() => void update().catch(reportError));
      if (disposed) {
        unlisten();
        return;
      }
      await update();
    })().catch(reportError);
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [reportError]);

  const run = (action: (current: Window) => Promise<void>) => {
    if (appWindow) void action(appWindow).catch(reportError);
  };
  return {
    desktop: appWindow !== null,
    maximized,
    minimize: () => run((current) => current.minimize()),
    toggleMaximize: () =>
      run(async (current) => {
        await current.toggleMaximize();
        setMaximized(await current.isMaximized());
      }),
    close: () => run((current) => current.close()),
  };
}
