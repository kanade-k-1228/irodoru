import clsx from "clsx";
import { type PrimitiveAtom, useAtomValue, useSetAtom } from "jotai";
import { GripVertical, Loader2, Plus, Trash2, UserRound } from "lucide-react";
import type { useRowDrag } from "@/logic/hooks/useRowDrag.ts";
import { characterStyle } from "@/logic/lib/character.ts";
import type { ScriptRow } from "@/logic/lib/script.ts";
import { busyAtom } from "@/logic/state/app.ts";
import { generationErrorsAtom, runningRowAtom } from "@/logic/state/audio.ts";
import { charactersAtom } from "@/logic/state/characters.ts";
import {
  canAddRowAtom,
  insertSelectedRowAtom,
  moveSelectedRowAtom,
  removeSelectedRowAtom,
  rowCountAtom,
  selectedRowIdAtom,
  selectRowAtom,
} from "@/logic/state/script.ts";

type Props = {
  rowAtom: PrimitiveAtom<ScriptRow>;
  index: number;
  rowDrag: ReturnType<typeof useRowDrag>;
  showInsert: boolean;
};

export const ScriptLine = ({ rowAtom, index, rowDrag, showInsert }: Props) => {
  const row = useAtomValue(rowAtom);
  const character = useAtomValue(charactersAtom).find((character) => character.id === row.characterId);
  const selected = useAtomValue(selectedRowIdAtom) === row.id;
  const running = useAtomValue(runningRowAtom) === row.id;
  const busy = useAtomValue(busyAtom);
  const canAdd = useAtomValue(canAddRowAtom);
  const rowCount = useAtomValue(rowCountAtom);
  const insert = useSetAtom(insertSelectedRowAtom);
  const error = useAtomValue(generationErrorsAtom)[row.id];
  const selectRow = useSetAtom(selectRowAtom);
  const select = (focus = false) => selectRow(row.id, focus);
  const move = useSetAtom(moveSelectedRowAtom);
  const remove = useSetAtom(removeSelectedRowAtom);
  const dragging = rowDrag.drag?.id === row.id;
  const dropPosition = rowDrag.drag?.target?.id === row.id ? rowDrag.drag.target.edge : undefined;
  const dragHandle = rowDrag.handle(row.id);
  return (
    <div
      className={clsx("script-row", selected && "is-active", running && "is-running", dragging && "is-dragging")}
      data-row-id={row.id}
      data-drop-position={dropPosition}
      style={characterStyle(character?.color)}
      onFocusCapture={() => {
        if (!selected) select();
      }}
    >
      <button
        {...dragHandle}
        type="button"
        className="row-drag-handle"
        disabled={busy}
        aria-label={`行${index + 1}を移動`}
        title="ドラッグで移動・↑↓で並べ替え"
        onKeyDown={(event) => {
          dragHandle.onKeyDown?.(event);
          if (event.key === "ArrowUp" || event.key === "ArrowDown") {
            event.preventDefault();
            move(event.key === "ArrowUp" ? -1 : 1);
          }
        }}
      >
        {running ? <Loader2 className="size-3.5 animate-spin" /> : <GripVertical className="size-3.5" />}
      </button>
      <div className="row-speaker" title={character?.name ?? "キャラ未選択"}>
        <span className="speaker-avatar text-xs" aria-hidden="true">
          {character?.icon ? (
            <img src={character.icon} alt="" />
          ) : (
            character?.name.slice(0, 1) || <UserRound className="size-4" />
          )}
        </span>
      </div>
      <button
        type="button"
        className={clsx("script-text", !row.text && "text-muted")}
        aria-label={`行${index + 1}を編集`}
        aria-pressed={selected}
        onClick={() => select(true)}
      >
        {row.text || "セリフを入力"}
      </button>
      {selected && (
        <button
          type="button"
          className="row-delete icon-btn btn-danger"
          aria-label="選択行を削除"
          title="削除"
          disabled={busy || rowCount <= 1}
          onClick={remove}
        >
          <Trash2 className="size-4" />
        </button>
      )}
      {error && (
        <p role="alert" className="col-span-full px-2 pb-1 text-xs text-warning">
          {error}
        </p>
      )}
      {showInsert && (
        <button
          type="button"
          className="row-insert"
          disabled={!canAdd}
          aria-label={`行${index + 1}の後にセリフを追加`}
          title="ここにセリフを追加"
          onClick={() => insert("new", row.id)}
        >
          <Plus className="size-3" />
        </button>
      )}
    </div>
  );
};
