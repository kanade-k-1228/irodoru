import type { GenerationSettings } from "@/logic/lib/script.ts";

type Props = {
  value: GenerationSettings;
  onChange: (value: GenerationSettings) => void;
  disabled: boolean;
};

export const SpeechSettings = ({ value, onChange, disabled }: Props) => {
  const change = <K extends keyof GenerationSettings>(key: K, v: GenerationSettings[K]) =>
    onChange({ ...value, [key]: v });
  return (
    <div className="speech-settings">
      <label className="setting-control setting-length">
        <span className="setting-label">
          長さ（倍率） <output>{value.durationScale.toFixed(2)}×</output>
        </span>
        <input
          type="range"
          aria-label="長さ"
          min={0.5}
          max={2}
          step={0.05}
          value={value.durationScale}
          disabled={disabled}
          onChange={(e) => change("durationScale", Number(e.target.value))}
        />
        <span className="range-end-labels" aria-hidden="true">
          <span>速く</span>
          <span>遅く</span>
        </span>
      </label>
    </div>
  );
};

export const AdvancedSpeechSettings = ({ value, onChange, disabled }: Props) => (
  <div className="advanced-settings">
    <div className="advanced-numbers">
      <label className="setting-control">
        <span className="label">シード</span>
        <input
          className="field"
          aria-label="シード"
          inputMode="numeric"
          value={value.seed}
          disabled={disabled}
          placeholder="ランダム"
          onChange={(event) => onChange({ ...value, seed: event.target.value.replace(/[^0-9]/g, "") })}
        />
      </label>
      <label className="setting-control">
        <span className="label">ステップ数</span>
        <input
          className="field"
          aria-label="ステップ数"
          inputMode="numeric"
          value={value.numSteps}
          disabled={disabled}
          placeholder="モデル既定（40）"
          onChange={(event) => onChange({ ...value, numSteps: event.target.value.replace(/[^0-9]/g, "") })}
        />
      </label>
    </div>
    {(
      [
        ["cfgText", "CFG 文"],
        ["cfgSpeaker", "CFG 声"],
        ["cfgCaption", "CFG 説明"],
      ] as const
    ).map(([key, label]) => (
      <label className="setting-control" key={key}>
        <span className="setting-label">
          {label}
          <output>{value[key]}</output>
        </span>
        <input
          type="range"
          aria-label={label}
          min={0}
          max={10}
          step={0.5}
          value={value[key]}
          disabled={disabled}
          onChange={(event) => onChange({ ...value, [key]: Number(event.target.value) })}
        />
      </label>
    ))}
  </div>
);
