import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

type Drive = {
  id: number;
  name: string;
  volumeUuid: string;
  mountPoint: string;
  isOnline: boolean;
  folders: string[];
};

type LibraryCounts = {
  series: number;
  untidySeries: number;
  episodes: number;
  movies: number;
  extras: number;
  unplacedFiles: number;
  files: number;
  probedFiles: number;
};

const config = {
  startFolder: "/Volumes/Glitchy_2TB/MEDIA",
  libraryKind: "cartoons",
};

export default function LibraryPanel() {
  const [drives, setDrives] = useState<Drive[] | null>(null);
  const [counts, setCounts] = useState<LibraryCounts | null>(null);
  const [reviewQueue, setReviewQueue] = useState<string[]>([]);
  const [isScanning, setIsScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function loadDrives() {
    try {
      setDrives(await invoke<Drive[]>("library_drives"));
    } catch (err) {
      setError(String(err));
    }
  }

  async function loadLibrary() {
    try {
      const [latestCounts, latestQueue] = await Promise.all([
        invoke<LibraryCounts>("library_counts"),
        invoke<string[]>("library_review_queue"),
      ]);
      setCounts(latestCounts);
      setReviewQueue(latestQueue);
    } catch (err) {
      setError(String(err));
    }
  }

  useEffect(() => {
    void loadDrives();
    void loadLibrary();
    const stopListening = listen("library-changed", () => {
      void loadDrives();
      void loadLibrary();
    });
    return () => {
      void stopListening.then((unlisten) => unlisten());
    };
  }, []);

  async function rescan() {
    setError(null);
    setIsScanning(true);
    try {
      await invoke("library_rescan");
    } catch (err) {
      setError(String(err));
    }
    setIsScanning(false);
  }

  async function addFolder() {
    const path = await open({ directory: true, defaultPath: config.startFolder });
    if (typeof path !== "string") return;

    setError(null);
    try {
      await invoke("library_add_folder", { path, kind: config.libraryKind });
      await loadDrives();
      await rescan();
    } catch (err) {
      setError(String(err));
    }
  }

  const hasDrives = drives && drives.length > 0;
  const hasReviewQueue = reviewQueue.length > 0;

  return (
    <section aria-label="Library" className="mb-6 max-w-3xl rounded bg-black/40 p-4 font-mono text-sm">
      <div className="mb-3 flex items-center justify-between gap-4">
        <h2 className="tracking-widest">LIBRARY DRIVES</h2>
        <div className="flex gap-2">
          <button
            type="button"
            className="min-w-32 rounded bg-plastic px-4 py-2"
            onClick={rescan}
            disabled={isScanning}
          >
            {isScanning ? "Scanning…" : "Rescan"}
          </button>
          <button type="button" className="rounded bg-dymo-blue px-4 py-2 font-bold" onClick={addFolder}>
            Add library folder
          </button>
        </div>
      </div>

      {drives === null && <p className="text-paper-muted">Checking drives…</p>}
      {drives && !hasDrives && <p className="text-paper-muted">No drives yet. Add MEDIA/Cartoons.</p>}

      {hasDrives && (
        <ul className="space-y-2">
          {drives.map((drive) => (
            <li key={drive.id}>
              <span className={drive.isOnline ? "text-vfd" : "text-paper-muted"}>
                {drive.isOnline ? "● online" : "○ offline"}
              </span>{" "}
              {drive.name} <span className="text-paper-muted">at {drive.mountPoint}</span>
              <div className="text-osd">{drive.volumeUuid}</div>
              <div className="text-paper-muted">Folders: {drive.folders.join(", ") || "none"}</div>
            </li>
          ))}
        </ul>
      )}

      {counts && (
        <p className="mt-4">
          {counts.series} shows ({counts.untidySeries} untidy) · {counts.episodes} episodes · {counts.movies} movies
          · {counts.extras} extras · <span className="text-osd">{counts.unplacedFiles} unplaced</span>
          <br />
          <span className="text-paper-muted">
            Running times read for {counts.probedFiles} of {counts.files} files
          </span>
        </p>
      )}

      {hasReviewQueue && (
        <details className="mt-3">
          <summary className="cursor-pointer text-paper-muted">Unplaced files ({reviewQueue.length})</summary>
          <ul className="mt-2 max-h-64 overflow-y-auto text-xs text-paper-muted">
            {reviewQueue.map((path) => (
              <li key={path}>{path}</li>
            ))}
          </ul>
        </details>
      )}

      {error && (
        <p role="alert" className="mt-3 rounded bg-dymo-red/30 p-3">
          {error}
        </p>
      )}
    </section>
  );
}
