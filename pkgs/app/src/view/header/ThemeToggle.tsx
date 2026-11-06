import { Moon, Sun } from "lucide-react";
import { useTheme } from "@/logic/hooks/useTheme.ts";

export const ThemeToggle = () => {
  const { dark, toggle } = useTheme();
  return (
    <button
      type="button"
      role="switch"
      className="theme-toggle"
      aria-label="ダークテーマ"
      aria-checked={dark}
      title={dark ? "ライトテーマに切り替え" : "ダークテーマに切り替え"}
      onClick={toggle}
    >
      <span className="theme-toggle-track" aria-hidden="true">
        <span className="theme-toggle-thumb" />
        <Sun className="theme-icon-light size-3.5" />
        <Moon className="theme-icon-dark size-3.5" />
      </span>
    </button>
  );
};
