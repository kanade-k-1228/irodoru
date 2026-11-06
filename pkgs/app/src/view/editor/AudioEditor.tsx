import { useAtomValue } from "jotai";
import { characterStyle } from "@/logic/lib/character.ts";
import { editingCharacterAtom } from "@/logic/state/characters.ts";
import { selectedCharacterAtom } from "@/logic/state/script.ts";
import { CharacterEditWorkspace } from "@/view/editor/character/CharacterEditor.tsx";
import { LineEditor } from "@/view/editor/line/LineEditor.tsx";
import { Characters } from "./Characters.tsx";

export const AudioEditor = () => {
  const character = useAtomValue(selectedCharacterAtom);
  const editing = useAtomValue(editingCharacterAtom);
  return (
    <section className="audio-editor editor-panel" aria-label="エディター" style={characterStyle(character?.color)}>
      {editing !== undefined ? (
        <CharacterEditWorkspace key={editing?.id ?? "new"} character={editing} />
      ) : (
        <>
          <Characters />
          <LineEditor />
        </>
      )}
    </section>
  );
};
