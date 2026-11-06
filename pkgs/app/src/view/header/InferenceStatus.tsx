import { useAtomValue } from "jotai";
import { inferenceStatusAtom } from "@/logic/state/app.ts";

export const InferenceStatus = () => {
  const status = useAtomValue(inferenceStatusAtom);
  const label =
    status.state === "error"
      ? status.error
      : ({ loading: "モデルを読み込み中", ready: "生成できます", generating: "生成中" }[status.state] ??
        "モデルを利用できません");
  return (
    <span className="inference-status" role="status" title={label}>
      anime · {label}
    </span>
  );
};
