import { useAtom, useAtomValue, useSetAtom } from "jotai";
import { Dices, Loader2, Settings2, WandSparkles } from "lucide-react";
import { type KeyboardEvent, useLayoutEffect, useRef } from "react";
import { busyAtom, canGenerateAtom } from "@/logic/state/app.ts";
import { generateSelectedRowAtom, runningRowAtom } from "@/logic/state/audio.ts";
import {
  advancedSettingsOpenAtom,
  editSelectedRowAtom,
  focusRowAtom,
  insertSelectedRowAtom,
  preferencesAtom,
  selectedCharacterAtom,
  selectedRowAtom,
  selectedRowIndexAtom,
} from "@/logic/state/script.ts";
import { AdvancedSpeechSettings, SpeechSettings } from "@/view/common/SpeechSettings.tsx";

export const LineControls = () => {
  const row = useAtomValue(selectedRowAtom);
  const index = useAtomValue(selectedRowIndexAtom);
  const busy = useAtomValue(busyAtom);
  const idle = useAtomValue(canGenerateAtom);
  const character = useAtomValue(selectedCharacterAtom);
  const defaults = useAtomValue(preferencesAtom);
  const running = useAtomValue(runningRowAtom) === row?.id;
  const onChange = useSetAtom(editSelectedRowAtom);
  const insert = useSetAtom(insertSelectedRowAtom);
  const generate = useSetAtom(generateSelectedRowAtom);
  const textRef = useRef<HTMLTextAreaElement>(null);
  const [focusRow, setFocusRow] = useAtom(focusRowAtom);
  const [advancedOpen, setAdvancedOpen] = useAtom(advancedSettingsOpenAtom);
  useLayoutEffect(() => {
    if (!focusRow || focusRow !== row?.id) return;
    textRef.current?.focus();
    textRef.current?.scrollIntoView({ block: "nearest" });
    setFocusRow(null);
  }, [focusRow, row?.id, setFocusRow]);
  const canGenerate = idle && !!character && !!row?.text.trim();
  const onTextKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.nativeEvent.isComposing || event.keyCode === 229) return;
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      if (canGenerate) void generate();
    } else if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      insert();
    }
  };
  if (!row) return null;
  return (
    <div className="line-controls">
      <textarea
        ref={textRef}
        className="field line-textarea resize-y"
        aria-label={`行${index + 1}のセリフ`}
        placeholder="セリフ"
        rows={3}
        maxLength={1000}
        value={row.text}
        disabled={busy}
        onChange={(event) => onChange({ text: event.target.value })}
        onKeyDown={onTextKeyDown}
      />
      <SpeechSettings
        value={row.settings ?? defaults}
        disabled={busy}
        onChange={(settings) => onChange({ settings })}
      />
      <div className="generation-actions">
        <button
          type="button"
          className="btn btn-primary flex-1"
          disabled={!canGenerate}
          onClick={() => void generate()}
        >
          {running ? <Loader2 className="size-3.5 animate-spin" /> : <WandSparkles className="size-3.5" />}
          {running ? "生成中…" : "生成"}
        </button>
        <button
          type="button"
          className="btn"
          aria-label="詳細設定"
          title="詳細設定"
          aria-expanded={advancedOpen}
          aria-controls="line-advanced-settings"
          onClick={() => setAdvancedOpen(!advancedOpen)}
        >
          <Settings2 className="size-4" />
        </button>
        <button
          type="button"
          className="btn"
          disabled={!canGenerate}
          aria-label="ランダム再生成"
          title="シードを変えて再生成"
          onClick={() => void generate(true)}
        >
          <Dices className="size-4" />
        </button>
      </div>
      {advancedOpen && (
        <section id="line-advanced-settings" aria-label="詳細設定">
          <AdvancedSpeechSettings
            value={row.settings ?? defaults}
            disabled={busy}
            onChange={(settings) => onChange({ settings })}
          />
        </section>
      )}
    </div>
  );
};
