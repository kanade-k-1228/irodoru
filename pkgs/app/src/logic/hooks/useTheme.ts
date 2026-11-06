import { useAtom } from "jotai";
import { useLayoutEffect } from "react";
import { themeAtom } from "@/logic/state/app.ts";

export const useTheme = () => {
  const [theme, setTheme] = useAtom(themeAtom);
  const dark = theme !== "light";
  useLayoutEffect(() => {
    document.documentElement.dataset.theme = dark ? "dark" : "light";
  }, [dark]);
  return { dark, toggle: () => setTheme(dark ? "light" : "dark") };
};
