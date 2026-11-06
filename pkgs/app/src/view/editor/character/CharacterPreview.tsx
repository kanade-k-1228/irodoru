import { useAtomValue } from "jotai";
import { Loader2, WandSparkles } from "lucide-react";
import { useCharacterPreview } from "@/logic/hooks/useCharacterPreview.ts";
import { exportPreview } from "@/logic/lib/api.ts";
import { canGenerateAtom } from "@/logic/state/app.ts";
import { AudioPlayer } from "@/view/common/AudioPlayer.tsx";
import { AdvancedSpeechSettings, SpeechSettings } from "@/view/common/SpeechSettings.tsx";

export type CharacterPreviewProps = {
  text: string;
  onTextChange: (text: string) => void;
  description: string;
  voice: string;
  disabled: boolean;
  onError: (error: string | null) => void;
};

export const CharacterPreview = (props: CharacterPreviewProps) => {
  const canGenerate = useAtomValue(canGenerateAtom);
  const { text, onTextChange, disabled, onError } = props;
  const { settings, setSettings, generating, sample, source, currentSettings, generate } = useCharacterPreview(props);
  return (
    <div className="character-preview">
      <label>
        <span className="label">サンプル文</span>
        <textarea
          aria-label="サンプル文"
          className="field block resize-y"
          rows={3}
          maxLength={1000}
          value={text}
          disabled={disabled}
          onChange={(event) => onTextChange(event.target.value)}
        />
      </label>
      <details className="preview-settings">
        <summary>生成設定</summary>
        <SpeechSettings value={settings} onChange={setSettings} disabled={disabled} />
        <details className="preview-settings">
          <summary>詳細設定（CFGは対応モデルのみ有効）</summary>
          <AdvancedSpeechSettings value={settings} onChange={setSettings} disabled={disabled} />
        </details>
      </details>
      <button
        type="button"
        className="btn btn-primary"
        disabled={disabled || !canGenerate || !text.trim()}
        onClick={() => void generate()}
      >
        {generating ? <Loader2 className="size-3.5 animate-spin" /> : <WandSparkles className="size-3.5" />}
        {generating ? "生成中…" : "サンプル生成"}
      </button>
      {sample && (
        <div>
          <span className="label">サンプル音声{sample.settings !== currentSettings && "（変更前）"}</span>
          <AudioPlayer
            key={source}
            source={source}
            onError={onError}
            download={{ save: () => exportPreview(sample.id), label: "サンプル音声を保存" }}
          />
        </div>
      )}
    </div>
  );
};
