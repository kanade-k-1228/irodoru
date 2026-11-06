import { useSetAtom } from "jotai";
import { useTakeAudio } from "@/logic/hooks/useTakeAudio.ts";
import { exportTake, type Take } from "@/logic/lib/api.ts";
import { errorAtom } from "@/logic/state/app.ts";
import { AudioPlayer } from "@/view/common/AudioPlayer.tsx";

export const TakePlayer = ({ take, enabled = true }: { take?: Take; enabled?: boolean }) => {
  const { source, loading } = useTakeAudio(take?.id, enabled);
  const onError = useSetAtom(errorAtom);
  return (
    <AudioPlayer
      key={source}
      source={source}
      loading={loading}
      seconds={take?.audio_seconds}
      onError={onError}
      download={take && { save: () => exportTake(take.id), label: "テイクを保存" }}
    />
  );
};
