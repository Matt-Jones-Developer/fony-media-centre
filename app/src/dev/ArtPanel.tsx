import { useEffect, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Sleeve from "../tape/Sleeve";
import Spine from "../tape/Spine";
import WallPosters from "../tape/WallPosters";
import type { SeriesArt } from "../types/art";


export default function ArtPanel() {
  const [allArt, setAllArt] = useState<SeriesArt[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [selectedSeason, setSelectedSeason] = useState<number | null>(null);
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
  const season = selected?.seasons.find((each) => each.number === selectedSeason) ?? selected?.seasons[0];
  const downloadedPosters = allArt.filter((show) => show.posterFile).length;

  function toggleShow(seriesId: number) {
    setSelectedId(seriesId === selectedId ? null : seriesId);
    setSelectedSeason(null);
  }

  return (
    <section aria-label="Art" className="mb-6 max-w-5xl rounded bg-black/40 p-4 font-mono text-sm">
      <h2 className="mb-1 tracking-widest">ART</h2>
      <p className="mb-3 text-paper-muted">
        Posters on disk for {downloadedPosters} of {allArt.length} matched shows. Pick one to see its tapes above the grid.
      </p>

      {selected && (
        <div className="mb-4 rounded bg-plastic-dark p-3">
          <h3 className="mb-2 tracking-widest">{selected.name}</h3>
          {selected.overview && <p className="mb-3 max-w-3xl text-xs text-paper-muted">{selected.overview}</p>}
          <div className="flex flex-wrap items-start gap-6">
            <ul aria-label="Spines" className="flex gap-2">
              {selected.seasons.map((each) => (
                <li key={each.number} className="h-72 w-12">
                  <button
                    type="button"
                    aria-label={`${selected.name}, ${each.name ?? `season ${each.number}`}`}
                    aria-pressed={each.number === season?.number}
                    className="h-full w-full transition-transform aria-pressed:-translate-y-3 motion-reduce:transition-none"
                    onClick={() => setSelectedSeason(each.number)}
                  >
                    <Spine
                      name={selected.name}
                      seasonNumber={each.number}
                      logoFile={selected.logoFile}
                      spineColour={selected.spineColour}
                    />
                  </button>
                </li>
              ))}
            </ul>
            <div className="w-40">
              <Sleeve name={selected.name} posterFile={selected.posterFile} />
            </div>
            {season && (
              <div className="w-80">
                <WallPosters
                  name={selected.name}
                  seasonNumber={season.number}
                  showPosterFile={selected.posterFile}
                  seasonPosterFile={season.posterFile}
                />
              </div>
            )}
          </div>
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
