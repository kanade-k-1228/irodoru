import { Moon, Sun } from "lucide-react";
import { useTheme } from "@/logic/hooks/useTheme.ts";

export const ThemeToggle = ({ menuItem = false }: { menuItem?: boolean }) => {
  const { dark, toggle } = useTheme();
  return (
    // biome-ignore lint/a11y/useAriaPropsSupportedByRole: Both switch and menuitemcheckbox support aria-checked.
    <button
      type="button"
      role={menuItem ? "menuitemcheckbox" : "switch"}
      className={menuItem ? "theme-toggle app-menu-item" : "theme-toggle"}
      aria-label="ダークテーマ"
      aria-checked={dark}
      title={dark ? "ライトテーマに切り替え" : "ダークテーマに切り替え"}
      onClick={toggle}
    >
      {menuItem && <span>ダークテーマ</span>}
      <span className="theme-toggle-track" aria-hidden="true">
        <span className="theme-toggle-thumb" />
        <Sun className="theme-icon-light size-3.5" />
        <Moon className="theme-icon-dark size-3.5" />
      </span>
    </button>
  );
};
