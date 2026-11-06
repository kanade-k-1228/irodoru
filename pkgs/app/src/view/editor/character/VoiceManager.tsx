import { Loader2, Upload } from "lucide-react";
import { useVoiceManager } from "@/logic/hooks/useVoiceManager.ts";

export type VoiceManagerProps = {
  value: string;
  onChange: (voice: string) => void;
  disabled: boolean;
  name: string;
  onBusy: (busy: boolean) => void;
  onError: (message: string | null) => void;
};

export const VoiceManager = (props: VoiceManagerProps) => {
  const { value, onChange, disabled } = props;
  const { voices, uploading, fileRef, register } = useVoiceManager(props);
  return (
    <div>
      <label className="label" htmlFor="reference-voice">
        参照音声
      </label>
      <div className="flex gap-1">
        <select
          id="reference-voice"
          className="field flex-1"
          value={value}
          disabled={disabled || uploading}
          onChange={(e) => onChange(e.target.value)}
        >
          <option value="">なし</option>
          {value && !voices.includes(value) && <option value={value}>見つからない音声：{value}</option>}
          {voices.map((v) => (
            <option key={v} value={v}>
              {v}
            </option>
          ))}
        </select>
        <button
          type="button"
          className="btn shrink-0"
          aria-label={uploading ? "登録中…" : "音声ファイルを登録"}
          title="音声ファイルを登録"
          disabled={disabled || uploading}
          onClick={() => fileRef.current?.click()}
        >
          {uploading ? <Loader2 className="size-3.5 animate-spin" /> : <Upload className="size-3.5" />}
        </button>
      </div>
      <input
        ref={fileRef}
        aria-label="リファレンス音声ファイル"
        type="file"
        accept="audio/*"
        className="hidden"
        disabled={disabled || uploading}
        onChange={(e) => {
          const file = e.target.files?.[0];
          if (file) void register(file);
        }}
      />
    </div>
  );
};
