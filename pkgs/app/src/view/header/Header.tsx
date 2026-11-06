import { useAtom, useAtomValue } from "jotai";
import { AudioLines, CircleAlert, Save, X } from "lucide-react";
import { errorAtom, readyAtom } from "@/logic/state/app.ts";
import { InferenceStatus } from "./InferenceStatus.tsx";
import { ThemeToggle } from "./ThemeToggle.tsx";

export const Header = ({ onSave }: { onSave: () => void }) => {
  const ready = useAtomValue(readyAtom);
  const [error, setError] = useAtom(errorAtom);
  return (
    <header className="studio-header">
      <h1 className="flex items-center gap-1.5 text-base font-bold tracking-tight">
        <AudioLines className="size-5 text-accent" />
        irodoru
      </h1>
      <button
        type="button"
        className="icon-btn ml-auto"
        disabled={!ready}
        aria-label="台本を保存"
        title="台本を保存"
        onClick={onSave}
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
          {error ? "ローカルデータを読み込めません。アプリを再起動してください。" : "ライブラリを読み込み中…"}
        </div>
      )}
    </header>
  );
};
