import { describe, expect, it, vi } from "vitest";
import { ProjectOpenRequests } from "./projectOpenRequests.ts";

function fixture() {
  const paths: string[] = [];
  let allowed = true;
  const take = vi.fn(async () => paths.splice(0));
  const open = vi.fn(async (_path: string) => true);
  const report = vi.fn();
  const deferred = vi.fn();
  const queue = new ProjectOpenRequests(take, () => allowed, open, report, deferred);
  return {
    queue,
    take,
    open,
    report,
    deferred,
    receive: (...next: string[]) => {
      paths.push(...next);
      queue.refresh();
    },
    allow: (next: boolean) => {
      allowed = next;
    },
  };
}

function promise<T>() {
  let resolve!: (value: T) => void;
  const value = new Promise<T>((done) => {
    resolve = done;
  });
  return { value, resolve };
}

describe("OS project open requests", () => {
  it("keeps cold-start requests until initialization and opens running requests in order", async () => {
    const f = fixture();
    f.receive("日本語 空白.iro");
    await Promise.resolve();
    expect(f.take).not.toHaveBeenCalled();
    f.queue.start();
    await vi.waitFor(() => expect(f.open).toHaveBeenCalledWith("日本語 空白.iro"));
    f.receive("second.iro", "third.iro");
    await vi.waitFor(() =>
      expect(f.open.mock.calls.map(([path]) => path)).toEqual(["日本語 空白.iro", "second.iro", "third.iro"]),
    );
  });

  it("defers while busy or editing and resumes without dropping the request", async () => {
    const f = fixture();
    f.allow(false);
    f.queue.start();
    f.receive("pending.iro");
    await vi.waitFor(() => expect(f.deferred).toHaveBeenCalled());
    expect(f.open).not.toHaveBeenCalled();
    f.allow(true);
    await f.queue.resume();
    expect(f.open).toHaveBeenCalledExactlyOnceWith("pending.iro");
  });

  it("serializes openings and keeps requests arriving during a transition", async () => {
    const f = fixture();
    const first = promise<boolean>();
    f.open.mockImplementationOnce(() => first.value);
    f.queue.start();
    f.receive("first.iro");
    await vi.waitFor(() => expect(f.open).toHaveBeenCalledTimes(1));
    f.receive("second.iro");
    await vi.waitFor(() => expect(f.take).toHaveBeenCalledTimes(3));
    expect(f.open).toHaveBeenCalledTimes(1);
    first.resolve(true);
    await vi.waitFor(() => expect(f.open).toHaveBeenCalledTimes(2));
  });

  it("survives a StrictMode stop/start while a request read is in flight", async () => {
    const f = fixture();
    const read = promise<string[]>();
    f.take.mockImplementationOnce(() => read.value);
    f.queue.start();
    await vi.waitFor(() => expect(f.take).toHaveBeenCalledTimes(1));
    f.queue.stop();
    f.queue.start();
    read.resolve(["once.iro"]);
    await vi.waitFor(() => expect(f.open).toHaveBeenCalledExactlyOnceWith("once.iro"));
    await vi.waitFor(() => expect(f.take).toHaveBeenCalledTimes(2));
    expect(f.open).toHaveBeenCalledTimes(1);
  });

  it("holds a request when a transition guard rejects it", async () => {
    const f = fixture();
    f.open.mockResolvedValueOnce(false);
    f.queue.start();
    await vi.waitFor(() => expect(f.take).toHaveBeenCalledTimes(1));
    f.receive("waiting.iro");
    await vi.waitFor(() => expect(f.open).toHaveBeenCalledTimes(1));
    await f.queue.resume();
    expect(f.open.mock.calls).toEqual([["waiting.iro"], ["waiting.iro"]]);
  });

  it("reports failed opens once and allows later requests", async () => {
    const f = fixture();
    const failure = new Error("保存できませんでした");
    f.open.mockRejectedValueOnce(failure);
    f.queue.start();
    f.receive("bad.iro", "next.iro");
    await vi.waitFor(() => expect(f.open).toHaveBeenCalledTimes(2));
    expect(f.report).toHaveBeenCalledExactlyOnceWith(failure);
    await f.queue.resume();
    expect(f.open).toHaveBeenCalledTimes(2);
  });
});
