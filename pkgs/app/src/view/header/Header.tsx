import { useAtom, useAtomValue } from "jotai";
import { AudioLines, CircleAlert, FilePlus2, FolderOpen, Save, X } from "lucide-react";
import type { useLibrary } from "@/logic/hooks/useLibrary.ts";
import { busyAtom, errorAtom, readyAtom } from "@/logic/state/app.ts";
import { editingCharacterAtom } from "@/logic/state/characters.ts";
import { InferenceStatus } from "./InferenceStatus.tsx";
import { ThemeToggle } from "./ThemeToggle.tsx";

export const Header = ({ project }: { project: ReturnType<typeof useLibrary> }) => {
  const ready = useAtomValue(readyAtom);
  const busy = useAtomValue(busyAtom);
  const editing = useAtomValue(editingCharacterAtom) !== undefined;
  const disabled = project.loading || busy || editing;
  const [error, setError] = useAtom(errorAtom);
  return (
    <header className="studio-header">
      <h1 className="flex items-center gap-1.5 text-base font-bold tracking-tight">
        <AudioLines className="size-5 text-accent" />
        irodoru
      </h1>
      <span className="ml-auto truncate text-xs text-muted" role="status">
        {project.status.name ?? "プロジェクト未選択"}
        {project.status.id &&
          ` · ${{ closed: "", saved: "保存済み", saving: "保存中…", error: "保存失敗" }[project.status.state] ?? ""}`}
      </span>
      <button
        type="button"
        className="icon-btn"
        disabled={disabled}
        aria-label="プロジェクトを新規作成"
        title="プロジェクトを新規作成"
        onClick={project.create}
      >
        <FilePlus2 className="size-4" />
      </button>
      <button
        type="button"
        className="icon-btn"
        disabled={disabled}
        aria-label="プロジェクトを開く"
        title="プロジェクトを開く"
        onClick={project.open}
      >
        <FolderOpen className="size-4" />
      </button>
      <button
        type="button"
        className="icon-btn"
        disabled={!ready}
        aria-label="プロジェクトを保存"
        title="プロジェクトを保存"
        onClick={project.save}
      >
        <Save className="size-4" />
      </button>
      <ThemeToggle />
      <InferenceStatus />
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
