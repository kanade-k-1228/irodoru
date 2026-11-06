import { useAtom, useAtomValue } from "jotai";
import { CircleAlert, Copy, Minus, Square, X } from "lucide-react";
import type { useLibrary } from "@/logic/hooks/useLibrary.ts";
import { useWindowControls } from "@/logic/hooks/useWindowControls.ts";
import { busyAtom, errorAtom, readyAtom } from "@/logic/state/app.ts";
import { editingCharacterAtom } from "@/logic/state/characters.ts";
import { AppMenu } from "./AppMenu.tsx";
import { InferenceStatus } from "./InferenceStatus.tsx";

export const Header = ({ project }: { project: ReturnType<typeof useLibrary> }) => {
  const ready = useAtomValue(readyAtom);
  const busy = useAtomValue(busyAtom);
  const editing = useAtomValue(editingCharacterAtom) !== undefined;
  const disabled = project.loading || busy || editing;
  const [error, setError] = useAtom(errorAtom);
  const windowControls = useWindowControls();
  return (
    <header className="studio-header">
      <div className="studio-toolbar" data-tauri-drag-region={windowControls.desktop || undefined}>
        <AppMenu project={project} windowControls={windowControls} disabled={disabled} ready={ready} />
        <span className="project-title" role="status" data-tauri-drag-region={windowControls.desktop || undefined}>
          {project.status.name ?? "プロジェクト未選択"}
          {project.status.id &&
            ` · ${{ closed: "", saved: "保存済み", saving: "保存中…", error: "保存失敗" }[project.status.state] ?? ""}`}
        </span>
        <InferenceStatus />
        {windowControls.desktop && (
          <fieldset className="window-controls" aria-label="ウィンドウ操作">
            <button
              type="button"
              className="window-control"
              aria-label="最小化"
              title="最小化"
              onClick={windowControls.minimize}
            >
              <Minus className="size-4" />
            </button>
            <button
              type="button"
              className="window-control"
              aria-label={windowControls.maximized ? "元のサイズに戻す" : "最大化"}
              title={windowControls.maximized ? "元のサイズに戻す" : "最大化"}
              onClick={windowControls.toggleMaximize}
            >
              {windowControls.maximized ? <Copy className="size-3.5" /> : <Square className="size-3.5" />}
            </button>
            <button
              type="button"
              className="window-control window-control-close"
              aria-label="ウィンドウを閉じる"
              title="閉じる"
              disabled={project.loading}
              onClick={windowControls.close}
            >
              <X className="size-4" />
            </button>
          </fieldset>
        )}
      </div>
      {error && (
        <div role="alert" className="error-banner basis-full">
          <CircleAlert className="size-4 shrink-0" />
          <span className="flex-1">{error}</span>
          <button className="icon-btn" aria-label="エラーを閉じる" onClick={() => setError(null)}>
            <X className="size-4" />
          </button>
        </div>
      )}
      {!ready && (
        <div className="basis-full px-2 py-1 text-xs text-muted">
          {project.loading
            ? "プロジェクトを読み込み中…"
            : "プロジェクトZIPを新規作成するか、既存のZIPを開いてください。"}
        </div>
      )}
    </header>
  );
};
