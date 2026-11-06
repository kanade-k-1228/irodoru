import { getVersion } from "@tauri-apps/api/app";
import { isTauri } from "@tauri-apps/api/core";
import { useSetAtom } from "jotai";
import { AudioLines, ChevronDown, X } from "lucide-react";
import { type KeyboardEvent, useCallback, useEffect, useRef, useState } from "react";
import type { useLibrary } from "@/logic/hooks/useLibrary.ts";
import type { useWindowControls } from "@/logic/hooks/useWindowControls.ts";
import { newWindow } from "@/logic/lib/api.ts";
import { errorAtom } from "@/logic/state/app.ts";
import { ThemeToggle } from "./ThemeToggle.tsx";

type Props = {
  project: ReturnType<typeof useLibrary>;
  windowControls: ReturnType<typeof useWindowControls>;
  disabled: boolean;
  ready: boolean;
};

export const AppMenu = ({ project, windowControls, disabled, ready }: Props) => {
  const [open, setOpen] = useState(false);
  const [dialog, setDialog] = useState<"preferences" | "about" | null>(null);
  const [opening, setOpening] = useState(false);
  const [version, setVersion] = useState<string | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const modal = useRef<HTMLDialogElement>(null);
  const initialItem = useRef<"first" | "last">("first");
  const setError = useSetAtom(errorAtom);
  const close = useCallback((restoreFocus = true) => {
    setOpen(false);
    if (restoreFocus) trigger.current?.focus();
  }, []);
  const items = () => Array.from(menu.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? []);
  useEffect(() => {
    if (!open) return;
    const buttons = Array.from(menu.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? []);
    (initialItem.current === "last" ? buttons.at(-1) : buttons[0])?.focus();
    const outside = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) close(false);
    };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [open, close]);
  useEffect(() => {
    if (!dialog) return;
    modal.current?.showModal();
    if (dialog === "about" && isTauri()) {
      void getVersion()
        .then(setVersion)
        .catch((cause: unknown) => setError(String(cause)));
    }
  }, [dialog, setError]);
  const createWindow = useCallback(() => {
    if (opening) return;
    setOpening(true);
    void newWindow()
      .catch((cause: unknown) => setError(cause instanceof Error ? cause.message : String(cause)))
      .finally(() => setOpening(false));
  }, [opening, setError]);
  useEffect(() => {
    const shortcut = (event: globalThis.KeyboardEvent) => {
      if (dialog || event.defaultPrevented || event.isComposing || event.altKey || !(event.ctrlKey || event.metaKey))
        return;
      const key = event.key.toLowerCase();
      let action: (() => void) | undefined;
      if (key === "n" && event.shiftKey && windowControls.desktop) action = createWindow;
      if (!event.shiftKey) {
        if (key === "n" && !disabled) action = project.create;
        if (key === "o" && !disabled) action = project.open;
        if (key === "s" && ready) action = project.save;
        if (key === ",") action = () => setDialog("preferences");
        if (key === "w" && windowControls.desktop && !project.loading) action = windowControls.close;
      }
      if (action) {
        event.preventDefault();
        close(open);
        action();
      }
    };
    document.addEventListener("keydown", shortcut);
    return () => document.removeEventListener("keydown", shortcut);
  }, [close, createWindow, dialog, disabled, open, project, ready, windowControls]);
  const select = (action: () => void) => () => {
    close();
    action();
  };
  const navigate = (event: KeyboardEvent<HTMLDivElement>) => {
    const buttons = items();
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    let next: number | undefined;
    if (event.key === "ArrowDown") next = (index + 1) % buttons.length;
    if (event.key === "ArrowUp") next = (index - 1 + buttons.length) % buttons.length;
    if (event.key === "Home") next = 0;
    if (event.key === "End") next = buttons.length - 1;
    if (next !== undefined) {
      event.preventDefault();
      buttons[next]?.focus();
    }
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      close();
    }
    if (event.key === "Tab") close();
  };
  return (
    <div className="app-menu" ref={root}>
      <button
        type="button"
        className="app-menu-trigger"
        ref={trigger}
        aria-label="irodoru メニュー"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? "app-menu" : undefined}
        onClick={() => {
          initialItem.current = "first";
          setOpen(!open);
        }}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault();
            initialItem.current = event.key === "ArrowUp" ? "last" : "first";
            setOpen(true);
          }
        }}
      >
        <AudioLines className="size-5 text-accent" />
        <span>irodoru</span>
        <ChevronDown className="size-3 text-muted" />
      </button>
      {open && (
        <div id="app-menu" className="app-menu-popup" role="menu" aria-label="irodoru" ref={menu} onKeyDown={navigate}>
          {windowControls.desktop && (
            <button
              type="button"
              role="menuitem"
              className="app-menu-item"
              disabled={opening}
              onClick={select(createWindow)}
            >
              新規ウィンドウ
            </button>
          )}
          <button
            type="button"
            role="menuitem"
            className="app-menu-item"
            disabled={disabled}
            onClick={select(project.create)}
          >
            新規プロジェクト…
          </button>
          <button
            type="button"
            role="menuitem"
            className="app-menu-item"
            disabled={disabled}
            onClick={select(project.open)}
          >
            プロジェクトを開く…
          </button>
          <button
            type="button"
            role="menuitem"
            className="app-menu-item"
            disabled={!ready}
            onClick={select(project.save)}
          >
            プロジェクトを保存
          </button>
          <hr className="app-menu-separator" />
          <ThemeToggle menuItem />
          <button
            type="button"
            role="menuitem"
            className="app-menu-item"
            onClick={select(() => setDialog("preferences"))}
          >
            環境設定…
          </button>
          <hr className="app-menu-separator" />
          <button type="button" role="menuitem" className="app-menu-item" onClick={select(() => setDialog("about"))}>
            irodoruについて
          </button>
          {windowControls.desktop && (
            <button
              type="button"
              role="menuitem"
              className="app-menu-item"
              disabled={project.loading}
              onClick={select(windowControls.close)}
            >
              ウィンドウを閉じる
            </button>
          )}
        </div>
      )}
      {dialog && (
        <dialog
          ref={modal}
          className="app-dialog"
          aria-labelledby="app-dialog-title"
          onClose={() => {
            setDialog(null);
            trigger.current?.focus();
          }}
        >
          <div className="flex items-center justify-between gap-4">
            <h2 id="app-dialog-title" className="font-bold">
              {dialog === "preferences" ? "環境設定" : "irodoruについて"}
            </h2>
            <button
              type="button"
              className="icon-btn"
              aria-label="ダイアログを閉じる"
              onClick={() => modal.current?.close()}
            >
              <X className="size-4" />
            </button>
          </div>
          {dialog === "preferences" ? (
            <div className="mt-5 flex items-center justify-between gap-8">
              <span className="text-sm">ダークテーマ</span>
              <ThemeToggle />
            </div>
          ) : (
            <div className="mt-5 space-y-2 text-sm">
              <p className="flex items-center gap-2 text-lg font-bold">
                <AudioLines className="size-6 text-accent" />
                irodoru
              </p>
              {version && <p className="text-muted">バージョン {version}</p>}
              <p>キャラクタの声で台本を読み上げる音声合成アプリ</p>
            </div>
          )}
        </dialog>
      )}
    </div>
  );
};
