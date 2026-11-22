import { useEffect, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type SeasonArt = {
  number: number;
  name: string | null;
  overview: string | null;
  posterFile: string | null;
};

type SeriesArt = {
  seriesId: number;
  name: string;
  overview: string | null;
  posterFile: string | null;
  logoFile: string | null;
  seasons: SeasonArt[];
};

export default function ArtPanel() {
  const [allArt, setAllArt] = useState<SeriesArt[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function loadArt() {
    try {
      setAllArt(await invoke<SeriesArt[]>("series_art"));
    } catch (err) {
      setError(String(err));
    }
  }

  useEffect(() => {
    void loadArt();
    const stopListening = listen("library-changed", () => void loadArt());
    return () => {
      void stopListening.then((unlisten) => unlisten());
    };
  }, []);

  const selected = allArt.find((show) => show.seriesId === selectedId);
  const downloadedPosters = allArt.filter((show) => show.posterFile).length;

  function toggleShow(seriesId: number) {
    setSelectedId(seriesId === selectedId ? null : seriesId);
  }

  return (
    <section aria-label="Art" className="mb-6 max-w-5xl rounded bg-black/40 p-4 font-mono text-sm">
      <h2 className="mb-1 tracking-widest">ART</h2>
      <p className="mb-3 text-paper-muted">
        Posters on disk for {downloadedPosters} of {allArt.length} matched shows. Pick one to see its seasons above the grid.
      </p>

      {selected && (
        <div className="mb-4 rounded bg-plastic-dark p-3">
          <h3 className="mb-2 tracking-widest">{selected.name}</h3>
          {selected.overview && <p className="mb-3 max-w-3xl text-xs text-paper-muted">{selected.overview}</p>}
          <ul className="grid grid-cols-[repeat(auto-fill,minmax(7rem,1fr))] gap-3">
            {selected.seasons.map((season) => (
              <li key={season.number} className="text-xs">
                <ArtImage
                  file={season.posterFile}
                  label={`${selected.name} season ${season.number} poster`}
                  className="aspect-2/3 w-full"
                />
                <p className="mt-1">{season.name ?? `Season ${season.number}`}</p>
              </li>
            ))}
          </ul>
        </div>
      )}

      <ul className="grid grid-cols-[repeat(auto-fill,minmax(7rem,1fr))] gap-3">
        {allArt.map((show) => (
          <li key={show.seriesId}>
            <button
              type="button"
              aria-pressed={show.seriesId === selectedId}
              className="w-full rounded bg-plastic-dark p-1 text-left aria-pressed:outline-2 aria-pressed:outline-vfd"
              onClick={() => toggleShow(show.seriesId)}
            >
              <ArtImage file={show.posterFile} label={`${show.name} poster`} className="aspect-2/3 w-full" />
              <ArtImage file={show.logoFile} label={`${show.name} logo`} className="mt-1 h-8 w-full object-contain" />
            </button>
          </li>
        ))}
      </ul>

      {error && (
        <p role="alert" className="mt-3 rounded bg-dymo-red/30 p-3">
          {error}
        </p>
      )}
    </section>
  );
}

interface ArtImageProps {
  file: string | null;
  label: string;
  className: string;
}

function ArtImage({ file, label, className }: ArtImageProps) {
  if (!file) {
    return (
      <div className={`${className} grid place-items-center bg-black/40 text-xs text-paper-muted`}>No image</div>
    );
  }
  return <img src={convertFileSrc(file)} alt={label} className={`${className} object-cover`} />;
}
