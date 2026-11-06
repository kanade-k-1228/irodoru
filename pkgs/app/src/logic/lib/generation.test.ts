import { beforeEach, expect, it, vi } from "vitest";
import { finishGeneration, generateTake, prepareGeneration, type Take, type TakeRequest } from "./api.ts";
import { generateQueue } from "./generation.ts";

vi.mock("./api.ts", () => ({ finishGeneration: vi.fn(), generateTake: vi.fn(), prepareGeneration: vi.fn() }));
const requests: TakeRequest[] = ["first", "second"].map((row_id) => ({
  row_id,
  character_id: "character",
  additional_prompt: null,
  request: {
    text: "こんにちは",
    seed: null,
    num_steps: null,
    cfg_scale_text: 3,
    cfg_scale_speaker: 5,
    cfg_scale_caption: 3,
    duration_scale: 1,
  },
}));
const callbacks = () => ({ shouldStop: () => false, onStart: vi.fn(), onTake: vi.fn(), onError: vi.fn() });
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(prepareGeneration).mockResolvedValue("batch");
});
it("stopping lets the in-flight take finish and skips remaining rows", async () => {
  let complete!: (take: Take) => void;
  let stop = false;
  vi.mocked(generateTake).mockImplementation(
    () =>
      new Promise((resolve) => {
        complete = resolve;
      }),
  );
  const cb = { ...callbacks(), shouldStop: () => stop };
  const running = generateQueue(requests, cb);
  await vi.waitFor(() => expect(generateTake).toHaveBeenCalledTimes(1));
  stop = true;
  const take = { id: "saved" } as Take;
  complete(take);
  await running;
  expect(cb.onTake).toHaveBeenCalledWith(take);
  expect(generateTake).toHaveBeenCalledTimes(1);
  expect(finishGeneration).toHaveBeenCalledWith("batch");
});
it("reports a failed row, continues, and always releases the snapshot", async () => {
  vi.mocked(generateTake)
    .mockRejectedValueOnce(new Error("inference failed"))
    .mockResolvedValueOnce({ id: "saved" } as Take);
  const cb = callbacks();
  await generateQueue(requests, cb);
  expect(cb.onError).toHaveBeenCalledWith("first", expect.any(Error));
  expect(cb.onTake).toHaveBeenCalledTimes(1);
  expect(generateTake).toHaveBeenNthCalledWith(2, "batch", 1);
  expect(finishGeneration).toHaveBeenCalledWith("batch");
});
it("does not generate any rows when snapshot creation fails", async () => {
  vi.mocked(prepareGeneration).mockRejectedValue(new Error("missing reference"));
  await expect(generateQueue(requests, callbacks())).rejects.toThrow("missing reference");
  expect(generateTake).not.toHaveBeenCalled();
  expect(finishGeneration).not.toHaveBeenCalled();
});
