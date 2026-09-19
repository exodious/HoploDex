import { useState } from "react";
import { SegmentedControl } from "../../components";
import { applyTheme, loadTheme } from "./theme";
import type { ThemePreference } from "./theme";

/** Auto follows the operating system's light/dark setting; the other two
 * override it. */
export function ThemeToggle() {
  const [theme, setTheme] = useState<ThemePreference>(loadTheme);

  return (
    <SegmentedControl<ThemePreference>
      label="Color mode"
      hideLabel
      size="sm"
      value={theme}
      onChange={(next) => {
        setTheme(next);
        applyTheme(next);
      }}
      options={[
        { value: "auto", label: "Auto (match system)", icon: "auto", iconOnly: true },
        { value: "light", label: "Light", icon: "sun", iconOnly: true },
        { value: "dark", label: "Dark", icon: "moon", iconOnly: true },
      ]}
    />
  );
}
