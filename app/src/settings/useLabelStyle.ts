import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type LabelStyle = "graf" | "regular";

/** The saved label style, kept on the page root so the `graf:` CSS variant applies everywhere. */
export function useLabelStyle() {
  const [labelStyle, setLabelStyle] = useState<LabelStyle>("graf");

  async function loadLabelStyle() {
    const saved = await invoke<LabelStyle>("label_style");
    setLabelStyle(saved);
    document.documentElement.dataset.labelStyle = saved;
  }

  useEffect(() => {
    void loadLabelStyle();
    const stopListening = listen("settings-changed", () => void loadLabelStyle());
    return () => {
      void stopListening.then((unlisten) => unlisten());
    };
  }, []);

  async function saveLabelStyle(style: LabelStyle) {
    await invoke("set_label_style", { style });
  }

  return { labelStyle, saveLabelStyle };
}
