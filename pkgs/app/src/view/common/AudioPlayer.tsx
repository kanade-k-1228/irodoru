import { Download, Loader2, Pause, Play } from "lucide-react";
import { formatTime, useAudioPlayer } from "@/logic/hooks/useAudioPlayer.ts";

type Props = {
  source?: string;
  loading?: boolean;
  seconds?: number;
  download?: { save: () => Promise<void>; label: string };
  onError: (message: string) => void;
};

export const AudioPlayer = ({ source, loading = false, seconds = 0, download, onError }: Props) => {
  const { audio, playing, position, duration, toggle, seek, mediaEvents } = useAudioPlayer(onError);
  return (
    <div className="take-player">
      {source && <audio ref={audio} src={source} preload="metadata" {...mediaEvents} />}
      <button
        type="button"
        className="transport-play"
        disabled={!source}
        aria-label={playing ? "一時停止" : "再生"}
        onClick={() => void toggle()}
      >
        {loading ? (
          <Loader2 className="size-4 animate-spin" />
        ) : playing ? (
          <Pause className="size-4" />
        ) : (
          <Play className="size-4" />
        )}
      </button>
      <span className="w-9 text-right font-mono text-xs text-muted">{formatTime(position)}</span>
      <input
        className="min-w-0 flex-1 accent-[var(--color-accent)]"
        type="range"
        aria-label="再生位置"
        min={0}
        max={duration || 1}
        step={0.01}
        value={Math.min(position, duration)}
        disabled={!duration}
        onChange={(e) => seek(Number(e.target.value))}
      />
      <span className="w-9 font-mono text-xs text-muted">{formatTime(duration || seconds || 0)}</span>
      {download && (
        <button
          className="icon-btn"
          type="button"
          onClick={() => void download.save().catch((error: Error) => onError(error.message))}
          aria-label={download.label}
          title="WAVを保存"
        >
          <Download className="size-4" />
        </button>
      )}
    </div>
  );
};
