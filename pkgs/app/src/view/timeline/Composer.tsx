import { useAtomValue, useSetAtom } from "jotai";
import { ChevronsDown, ChevronsUp, Square } from "lucide-react";
import { useEffect, useState } from "react";
import { useRowDrag } from "@/logic/hooks/useRowDrag.ts";
import { runningRowAtom, stopGenerationAtom, stoppingAtom } from "@/logic/state/audio.ts";
import { reorderRowsAtom, rowAtomsAtom, rowCountAtom } from "@/logic/state/script.ts";
import { ScriptLine } from "./ScriptLine.tsx";

export const Composer = () => {
  const rowAtoms = useAtomValue(rowAtomsAtom);
  const count = useAtomValue(rowCountAtom);
  const running = useAtomValue(runningRowAtom);
  const stopping = useAtomValue(stoppingAtom);
  const stop = useSetAtom(stopGenerationAtom);
  const reorder = useSetAtom(reorderRowsAtom);
  const rowDrag = useRowDrag(reorder);
  const [scroll, setScroll] = useState({ top: 0, max: 0 });
  useEffect(() => {
    const list = rowDrag.list.current;
    if (!list) return;
    const update = () => setScroll({ top: list.scrollTop, max: Math.max(0, list.scrollHeight - list.clientHeight) });
    const observer = new ResizeObserver(update);
    const observeRows = () => {
      observer.disconnect();
      observer.observe(list);
      for (const child of list.children) observer.observe(child);
      update();
    };
    const mutations = new MutationObserver(observeRows);
    mutations.observe(list, { childList: true });
    observeRows();
    list.addEventListener("scroll", update);
    return () => {
      observer.disconnect();
      mutations.disconnect();
      list.removeEventListener("scroll", update);
    };
  }, [rowDrag.list]);
  return (
    <main className="script-editor editor-panel" aria-label="台本エディタ">
      <div className="script-scroll-region">
        <button
          type="button"
          className="script-scroll-button script-scroll-top"
          aria-label="台本の先頭へ"
          title="先頭へ"
          onClick={() => rowDrag.list.current?.scrollTo({ top: 0 })}
        >
          <ChevronsUp className="size-4" />
        </button>
        <input
          className="script-scrollbar"
          type="range"
          aria-label="台本のスクロール位置"
          min={0}
          max={scroll.max}
          value={scroll.top}
          disabled={scroll.max === 0}
          onChange={(event) => {
            if (rowDrag.list.current) rowDrag.list.current.scrollTop = Number(event.target.value);
          }}
        />
        <section ref={rowDrag.list} className="script-list" aria-label="セリフ一覧">
          {rowAtoms.map((rowAtom, index) => (
            <ScriptLine
              key={rowAtom.toString()}
              rowAtom={rowAtom}
              index={index}
              rowDrag={rowDrag}
              showInsert={index < count - 1}
            />
          ))}
        </section>
        <button
          type="button"
          className="script-scroll-button script-scroll-bottom"
          aria-label="台本の末尾へ"
          title="末尾へ"
          onClick={() => rowDrag.list.current?.scrollTo({ top: rowDrag.list.current.scrollHeight })}
        >
          <ChevronsDown className="size-4" />
        </button>
      </div>
      {running && (
        <div className="script-actions">
          <button className="btn" disabled={stopping} onClick={stop}>
            <Square className="size-3" />
            {stopping ? "停止待ち" : "停止"}
          </button>
        </div>
      )}
    </main>
  );
};
