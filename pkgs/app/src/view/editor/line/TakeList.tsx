import clsx from "clsx";
import { useAtomValue, useSetAtom } from "jotai";
import { modelLabel, takeLabel } from "@/logic/lib/take.ts";
import { selectedRowTakesAtom, selectedTakeAtom } from "@/logic/state/audio.ts";
import { editSelectedRowAtom, selectedRowAtom } from "@/logic/state/script.ts";
import { TakePlayer } from "./TakePlayer.tsx";

export const TakeList = () => {
  const row = useAtomValue(selectedRowAtom);
  const takes = useAtomValue(selectedRowTakesAtom);
  const selectedTake = useAtomValue(selectedTakeAtom);
  const onChange = useSetAtom(editSelectedRowAtom);
  if (!row) return null;
  return (
    <section className="line-takes" aria-label="テイク">
      {takes.map((take, index) => (
        <fieldset
          key={take.id}
          className={clsx("line-take", selectedTake?.id === take.id && "is-selected")}
          aria-label={takeLabel(takes.length - index)}
          title={`${modelLabel(take.model)} · seed ${take.seed} · ${take.num_steps} steps${take.text !== row.text ? `\n${take.text}` : ""}`}
        >
          <input
            type="radio"
            name={`take-${row.id}`}
            className="take-choice accent-[var(--color-accent)]"
            aria-label={`テイク${takes.length - index}を選択`}
            checked={selectedTake?.id === take.id}
            onChange={() => onChange({ takeId: take.id })}
          />
          <TakePlayer take={take} />
        </fieldset>
      ))}
    </section>
  );
};
