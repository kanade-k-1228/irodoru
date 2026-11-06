import { Loader2, Save, Trash2, UserRound, X } from "lucide-react";
import type { ReactNode } from "react";
import { useCharacterEditor } from "@/logic/hooks/useCharacterEditor.ts";
import type { Character } from "@/logic/lib/api.ts";
import { characterStyle } from "@/logic/lib/character.ts";
import { prepareCharacterIcon } from "@/logic/lib/icon.ts";
import { Characters } from "@/view/editor/Characters.tsx";
import { CharacterPreview } from "./CharacterPreview.tsx";
import { VoiceManager } from "./VoiceManager.tsx";

export const CharacterEditWorkspace = ({ character }: { character: Character | null }) => {
  const editor = useCharacterEditor(character);
  const { draft, saving, dirty, disabled, close, remove } = editor;
  const actions = (
    <span className="character-edit-actions" style={characterStyle(draft.color)}>
      {character && (
        <button
          type="button"
          className="icon-btn btn-danger"
          disabled={disabled}
          aria-label="キャラを削除"
          title="キャラを削除"
          onClick={() => void remove()}
        >
          <Trash2 className="size-4" />
        </button>
      )}
      <button
        type="button"
        className="icon-btn"
        disabled={disabled}
        aria-label="キャンセル"
        title="キャンセル"
        onClick={close}
      >
        <X className="size-4" />
      </button>
      <button
        type="submit"
        form="character-form"
        className="icon-btn character-save"
        disabled={disabled || !draft.name.trim() || !dirty}
        aria-label="保存"
        title="保存"
      >
        {saving ? <Loader2 className="size-4 animate-spin" /> : <Save className="size-4" />}
      </button>
    </span>
  );
  return (
    <>
      <Characters />
      <CharacterEditor editor={editor} actions={actions} isNew={!character} />
    </>
  );
};

const CharacterEditor = ({
  editor,
  actions,
  isNew,
}: {
  editor: ReturnType<typeof useCharacterEditor>;
  actions: ReactNode;
  isNew: boolean;
}) => {
  const { nameInput, draft, error, disabled, change, setUploading, setError, save } = editor;
  return (
    <section className="character-editor" aria-label="キャラ編集" style={characterStyle(draft.color)}>
      <div className="character-content-toolbar">
        <span className="min-w-0 flex-1 truncate">{isNew ? "キャラ追加" : "キャラ編集"}</span>
        {actions}
      </div>
      {error && (
        <div role="alert" className="error-banner">
          {error}
        </div>
      )}
      <form
        id="character-form"
        className="character-form"
        onSubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <div className="character-dialog-body">
          <div className="character-fields">
            <div className="character-identity">
              <label className="character-icon-picker" title="画像を選択">
                <span className="speaker-avatar character-avatar" aria-hidden="true">
                  {draft.icon ? (
                    <img src={draft.icon} alt="" />
                  ) : (
                    draft.name.trim().slice(0, 1) || <UserRound className="size-6" />
                  )}
                </span>
                <input
                  type="file"
                  className="sr-only"
                  accept="image/*"
                  aria-label="キャラクター画像を選択"
                  disabled={disabled}
                  onChange={(event) => {
                    const file = event.currentTarget.files?.[0];
                    event.currentTarget.value = "";
                    if (file) {
                      setUploading(true);
                      setError(null);
                      void prepareCharacterIcon(file)
                        .then((icon) => change({ icon }))
                        .catch((error: Error) => setError(error.message))
                        .finally(() => setUploading(false));
                    }
                  }}
                />
              </label>
              {draft.icon && (
                <button
                  type="button"
                  className="icon-btn btn-danger"
                  aria-label="キャラクター画像を削除"
                  title="画像を削除"
                  disabled={disabled}
                  onClick={() => change({ icon: "" })}
                >
                  <X className="size-4" />
                </button>
              )}
              <label className="min-w-0 flex-1">
                <span className="label">名前</span>
                <input
                  ref={nameInput}
                  aria-label="名前"
                  className="field"
                  value={draft.name}
                  maxLength={64}
                  disabled={disabled}
                  onChange={(event) => change({ name: event.target.value })}
                  placeholder="さくら"
                />
              </label>
              <label className="shrink-0">
                <span className="label">色</span>
                <input
                  type="color"
                  className="character-color"
                  aria-label="テーマカラー"
                  value={draft.color}
                  disabled={disabled}
                  onChange={(event) => change({ color: event.target.value })}
                />
              </label>
            </div>
            <label>
              <span className="label">プロンプト</span>
              <textarea
                aria-label="プロンプト"
                className="field block resize-y"
                rows={5}
                value={draft.description}
                maxLength={2000}
                disabled={disabled}
                onChange={(event) => change({ description: event.target.value })}
                placeholder="落ち着いた低めの声"
              />
            </label>
            <VoiceManager
              value={draft.voice}
              name={draft.name}
              disabled={disabled}
              onBusy={setUploading}
              onError={setError}
              onChange={(voice) => change({ voice })}
            />
          </div>
          <CharacterPreview
            text={draft.sample_text}
            onTextChange={(sample_text) => change({ sample_text })}
            description={draft.description}
            voice={draft.voice}
            disabled={disabled}
            onError={setError}
          />
        </div>
      </form>
    </section>
  );
};
