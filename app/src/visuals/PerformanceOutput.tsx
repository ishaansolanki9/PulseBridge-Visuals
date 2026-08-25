import { useEffect, useState } from "react";

import { performanceSettings, visualSettingsStorageKey } from "../control/transport";
import { PerformanceCanvas } from "./PerformanceCanvas";

export function PerformanceOutput() {
  const [settings, setSettings] = useState(performanceSettings);
  useEffect(() => {
    const exit = (event: KeyboardEvent) => {
      if (event.key === "Escape") window.close();
    };
    const syncSettings = (event: StorageEvent) => {
      if (event.key === visualSettingsStorageKey) setSettings(performanceSettings());
    };
    window.addEventListener("keydown", exit);
    window.addEventListener("storage", syncSettings);
    return () => {
      window.removeEventListener("keydown", exit);
      window.removeEventListener("storage", syncSettings);
    };
  }, []);
  return (
    <main className="performance-output">
      <PerformanceCanvas settings={settings} />
    </main>
  );
}
