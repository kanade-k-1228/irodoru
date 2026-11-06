import { finishGeneration, generateTake, prepareGeneration, type Take, type TakeRequest } from "./api.ts";

export async function generateQueue(
  requests: TakeRequest[],
  callbacks: {
    shouldStop: () => boolean;
    onStart: (rowId: string) => void;
    onTake: (take: Take) => void;
    onError: (rowId: string, error: Error) => void;
  },
): Promise<void> {
  const batch = await prepareGeneration(requests);
  try {
    for (const [index, request] of requests.entries()) {
      if (callbacks.shouldStop()) break;
      callbacks.onStart(request.row_id);
      try {
        callbacks.onTake(await generateTake(batch, index));
      } catch (error) {
        callbacks.onError(request.row_id, error as Error);
      }
    }
  } finally {
    await finishGeneration(batch);
  }
}
