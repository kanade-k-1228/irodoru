import clsx from "clsx";
import { useAtom, useAtomValue, useSetAtom } from "jotai";
import { Pencil, Plus } from "lucide-react";
import { useLayoutEffect, useRef } from "react";
import { characterStyle } from "@/logic/lib/character.ts";
import { idleAtom } from "@/logic/state/app.ts";
import { activeCharacterAtom, charactersAtom, editingCharacterAtom } from "@/logic/state/characters.ts";
import { appendCharacterRowAtom } from "@/logic/state/script.ts";

export const Characters = () => {
  const characters = useAtomValue(charactersAtom);
  const selected = useAtomValue(activeCharacterAtom);
  const [editing, setEditing] = useAtom(editingCharacterAtom);
  const disabled = !useAtomValue(idleAtom) || editing !== undefined;
  const addLine = useSetAtom(appendCharacterRowAtom);
  const list = useRef<HTMLElement>(null);
  const selectedId = editing?.id ?? selected?.id;
  useLayoutEffect(() => {
    if (!selectedId || editing === null) return;
    const container = list.current;
    const tab = container?.querySelector<HTMLElement>(".character-tab.is-selected");
    if (!container || !tab) return;
    const bounds = container.getBoundingClientRect();
    const selectedBounds = tab.getBoundingClientRect();
    if (selectedBounds.top < bounds.top) container.scrollTop += selectedBounds.top - bounds.top;
    else if (selectedBounds.bottom > bounds.bottom) container.scrollTop += selectedBounds.bottom - bounds.bottom;
  }, [selectedId, editing]);
  return (
    <aside className="character-rail" aria-label="キャラクター">
      <nav ref={list} className="character-list" aria-label="キャラクタープリセット">
        {characters.map((character) => (
          <div
            key={character.id}
            className={clsx("character-tab", selectedId === character.id && "is-selected")}
            style={characterStyle(character.color)}
          >
            <button
              type="button"
              disabled={disabled}
              aria-pressed={selectedId === character.id}
              className="character-card"
              title={`${character.name}の行を追加`}
              aria-label={character.name}
              onClick={() => addLine(character.id)}
            >
              <span className="speaker-avatar" aria-hidden="true">
                {character.icon ? <img src={character.icon} alt="" /> : character.name.slice(0, 1)}
              </span>
              <span className="character-card-name">{character.name}</span>
            </button>
          </div>
        ))}
      </nav>
      <div className="character-actions">
        <button
          type="button"
          className="icon-btn"
          disabled={disabled}
          aria-label="キャラを追加"
          title="キャラを追加"
          onClick={() => setEditing(null)}
        >
          <Plus className="size-5" />
        </button>
        <button
          type="button"
          className="icon-btn"
          disabled={disabled || !selected}
          aria-label="キャラを編集"
          title={selected ? `${selected.name}を編集` : "キャラを編集"}
          onClick={() => selected && setEditing(selected)}
        >
          <Pencil className="size-4" />
        </button>
      </div>
    </aside>
  );
};
