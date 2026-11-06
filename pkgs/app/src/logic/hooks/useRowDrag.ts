import { type KeyboardEvent, type PointerEvent, useEffect, useRef, useState } from "react";

type Target = { id: string; edge: "before" | "after" };
type Gesture = { id: string; startX: number; startY: number; x: number; y: number; moved: boolean };

const inside = (bounds: DOMRect, { x, y }: Gesture) =>
  x >= bounds.left && x <= bounds.right && y >= bounds.top && y <= bounds.bottom;

export const useRowDrag = (onDrop: (id: string, targetId: string, edge: Target["edge"]) => void) => {
  const list = useRef<HTMLElement>(null);
  const gesture = useRef<Gesture | null>(null);
  const frame = useRef<number | null>(null);
  const [drag, setDrag] = useState<{ id: string; target?: Target } | null>(null);

  useEffect(
    () => () => {
      if (frame.current !== null) cancelAnimationFrame(frame.current);
    },
    [],
  );

  const targetAt = (current: Gesture): Target | undefined => {
    const container = list.current;
    if (!container || !inside(container.getBoundingClientRect(), current)) return;
    const rows = Array.from(container.querySelectorAll<HTMLElement>("[data-row-id]"));
    const row = rows.find((row) => current.y < row.getBoundingClientRect().bottom) ?? rows.at(-1);
    if (row?.dataset.rowId && row.dataset.rowId !== current.id) {
      const box = row.getBoundingClientRect();
      return { id: row.dataset.rowId, edge: current.y < box.top + box.height / 2 ? "before" : "after" };
    }
  };

  const track = () => {
    const current = gesture.current;
    const container = list.current;
    if (!current?.moved || !container) return;
    const bounds = container.getBoundingClientRect();
    if (inside(bounds, current)) {
      const margin = 32;
      const speed = current.y < bounds.top + margin ? -8 : current.y > bounds.bottom - margin ? 8 : 0;
      container.scrollTop += speed;
    }
    const target = targetAt(current);
    setDrag((previous) =>
      previous?.id === current.id && previous.target?.id === target?.id && previous.target?.edge === target?.edge
        ? previous
        : { id: current.id, target },
    );
    frame.current = requestAnimationFrame(track);
  };

  const reset = () => {
    gesture.current = null;
    if (frame.current !== null) cancelAnimationFrame(frame.current);
    frame.current = null;
    setDrag(null);
  };

  const finish = (event: PointerEvent<HTMLButtonElement>, drop: boolean) => {
    const current = gesture.current;
    const target = current?.moved && targetAt({ ...current, x: event.clientX, y: event.clientY });
    reset();
    if (event.currentTarget.hasPointerCapture(event.pointerId))
      event.currentTarget.releasePointerCapture(event.pointerId);
    if (drop && current && target) onDrop(current.id, target.id, target.edge);
  };

  const handle = (id: string) => ({
    onPointerDown: (event: PointerEvent<HTMLButtonElement>) => {
      if (event.button !== 0 || !event.isPrimary) return;
      const { clientX: x, clientY: y } = event;
      gesture.current = { id, startX: x, startY: y, x, y, moved: false };
      event.currentTarget.setPointerCapture(event.pointerId);
    },
    onPointerMove: (event: PointerEvent<HTMLButtonElement>) => {
      const current = gesture.current;
      if (!current) return;
      const next = { ...current, x: event.clientX, y: event.clientY };
      next.moved ||= Math.hypot(next.x - next.startX, next.y - next.startY) >= 4;
      gesture.current = next;
      if (!current.moved && next.moved) track();
    },
    onPointerUp: (event: PointerEvent<HTMLButtonElement>) => finish(event, true),
    onPointerCancel: (event: PointerEvent<HTMLButtonElement>) => finish(event, false),
    onLostPointerCapture: (event: PointerEvent<HTMLButtonElement>) => finish(event, false),
    onKeyDown: (event: KeyboardEvent<HTMLButtonElement>) => {
      if (event.key === "Escape") reset();
    },
  });

  return { list, drag, handle };
};
