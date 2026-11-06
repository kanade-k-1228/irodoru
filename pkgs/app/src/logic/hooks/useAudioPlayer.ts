import { type SyntheticEvent, useRef, useState } from "react";

export const formatTime = (seconds: number) =>
  `${Math.floor(seconds / 60)}:${String(Math.floor(seconds % 60)).padStart(2, "0")}`;

export const useAudioPlayer = (onError: (message: string) => void) => {
  const audio = useRef<HTMLAudioElement>(null);
  const [playing, setPlaying] = useState(false);
  const [position, setPosition] = useState(0);
  const [duration, setDuration] = useState(0);
  const toggle = async () => {
    if (!audio.current) return;
    if (playing) audio.current.pause();
    else {
      try {
        await audio.current.play();
      } catch {
        onError("音声を再生できませんでした");
      }
    }
  };
  const seek = (value: number) => {
    if (audio.current) audio.current.currentTime = value;
    setPosition(value);
  };
  const mediaEvents = {
    onLoadedMetadata: (event: SyntheticEvent<HTMLAudioElement>) =>
      setDuration(Number.isFinite(event.currentTarget.duration) ? event.currentTarget.duration : 0),
    onTimeUpdate: (event: SyntheticEvent<HTMLAudioElement>) => setPosition(event.currentTarget.currentTime),
    onPlay: () => setPlaying(true),
    onPause: () => setPlaying(false),
    onEnded: () => setPlaying(false),
    onError: () => onError("音声を読み込めませんでした"),
  };
  return { audio, playing, position, duration, toggle, seek, mediaEvents };
};
