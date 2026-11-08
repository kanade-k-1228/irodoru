import { useAtom } from "jotai";
import { RotateCcw } from "lucide-react";
import { useState } from "react";
import { retryInference } from "@/logic/lib/api.ts";
import { inferenceStatusAtom } from "@/logic/state/app.ts";

function bytes(value: number) {
  return value >= 1_000_000_000 ? `${(value / 1_000_000_000).toFixed(2)} GB` : `${(value / 1_000_000).toFixed(1)} MB`;
}

export const InferenceStatus = () => {
  const [status, setStatus] = useAtom(inferenceStatusAtom);
  const [retrying, setRetrying] = useState(false);
  const label =
    status.state === "error"
      ? status.error
      : status.state === "downloading"
        ? `モデルをダウンロード中 ${bytes(status.downloaded)} / ${bytes(status.total)}`
        : status.state === "extracting"
          ? `モデルを展開中 ${bytes(status.extracted)} / ${bytes(status.total)}`
          : status.state === "checking"
            ? `モデルを確認中 ${Math.floor((status.checked / status.total) * 100)}%`
            : { loading: "モデルを読み込み中", ready: "生成できます", generating: "生成中" }[status.state];
  const retry = async () => {
    setRetrying(true);
    try {
      await retryInference();
      setStatus({ state: "loading" });
    } catch (error) {
      setStatus({ state: "error", error: error instanceof Error ? error.message : String(error) });
    } finally {
      setRetrying(false);
    }
  };
  return (
    <div className={status.state === "error" ? "error-banner basis-full" : "flex min-w-0 items-center gap-1"}>
      <span
        className={status.state === "error" ? "min-w-0 flex-1 break-words text-xs" : "inference-status"}
        role="status"
        title={label}
      >
        anime · {label}
      </span>
      {(status.state === "downloading" || status.state === "extracting") && (
        <progress
          className="h-1 w-16 accent-accent"
          aria-label={status.state === "downloading" ? "モデルのダウンロード" : "モデルの展開"}
          value={status.state === "downloading" ? status.downloaded : status.extracted}
          max={status.total}
        />
      )}
      {status.state === "error" && (
        <button
          type="button"
          className="icon-btn"
          disabled={retrying}
          onClick={retry}
          title="モデルの準備を再試行"
          aria-label="モデルの準備を再試行"
        >
          <RotateCcw className="size-4" />
        </button>
      )}
    </div>
  );
};
