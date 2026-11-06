import { useAtomValue } from "jotai";
import { AudioLines } from "lucide-react";
import { selectedRowIdAtom } from "@/logic/state/script.ts";
import { LineControls } from "./LineControls.tsx";
import { TakeList } from "./TakeList.tsx";

export const LineEditor = () => {
  const rowId = useAtomValue(selectedRowIdAtom);
  return (
    <div className="editor-scroll">
      {rowId ? (
        <>
          <LineControls />
          <TakeList />
        </>
      ) : (
        <div className="empty-state">
          <AudioLines className="mx-auto mb-1 size-5 text-muted" />
          <p>台本の行を選択</p>
        </div>
      )}
    </div>
  );
};
