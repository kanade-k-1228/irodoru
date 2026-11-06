import { useAtomValue } from "jotai";
import { modelLabel } from "@/logic/lib/take.ts";
import { selectedTakeAtom } from "@/logic/state/audio.ts";
import { TakePlayer } from "./TakePlayer.tsx";

export const TakeList = () => {
  const take = useAtomValue(selectedTakeAtom);
  if (!take) return null;
  return (
    <section className="line-takes" aria-label="生成音声">
      <fieldset
        className="line-take is-selected"
        title={`${modelLabel(take.model)} · seed ${take.seed} · ${take.num_steps} steps`}
      >
        <TakePlayer take={take} />
      </fieldset>
    </section>
  );
};
