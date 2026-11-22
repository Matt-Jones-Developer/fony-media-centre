import { useEffect, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Sleeve from "../tape/Sleeve";
import Spine from "../tape/Spine";
import { seasonLabel } from "../tape/seasonLabel";
import WallPosters from "../tape/WallPosters";
import type { SeriesArt } from "../types/art";
import { useLabelStyle } from "../settings/useLabelStyle";
import type { LabelStyle } from "../settings/useLabelStyle";


export default function ArtPanel() {
  const [allArt, setAllArt] = useState<SeriesArt[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [selectedSeason, setSelectedSeason] = useState<number | null>(null);
  const [selectedMovieId, setSelectedMovieId] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { labelStyle, saveLabelStyle } = useLabelStyle();
  const labelStyles: { value: LabelStyle; name: string }[] = [
    { value: "regular", name: "Regular" },
    { value: "graf", name: "Graf" },
  ];

  async function pickLabelStyle(style: LabelStyle) {
    try {
      await saveLabelStyle(style);
    } catch (err) {
      setError(String(err));
    }
  }

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
  const movie = selected?.movies.find((each) => each.movieId === selectedMovieId);
  const seasonName = season?.name ?? `season ${season?.number}`;
  const tapeLabel = movie ? movie.title : `${selected?.name} ${seasonName}`;
  const tapePosterFile = movie ? movie.posterFile : (season?.posterFile ?? null);
  const hasTape = movie || season;

  function pickSeason(seasonNumber: number) {
    setSelectedSeason(seasonNumber);
    setSelectedMovieId(null);
  }
  const downloadedPosters = allArt.filter((show) => show.posterFile).length;

  function toggleShow(seriesId: number) {
    setSelectedId(seriesId === selectedId ? null : seriesId);
    setSelectedSeason(null);
    setSelectedMovieId(null);
  }

  return (
    <section aria-label="Art" className="mb-6 max-w-5xl rounded bg-black/40 p-4 font-mono text-sm">
      <div className="mb-1 flex items-center justify-between gap-4">
        <h2 className="tracking-widest">ART</h2>
        <div role="group" aria-label="Handwritten label style" className="flex gap-2">
          {labelStyles.map((style) => (
            <button
              key={style.value}
              type="button"
              aria-pressed={style.value === labelStyle}
              className="min-w-24 rounded bg-dymo-black px-3 py-2 font-bold aria-pressed:bg-dymo-red"
              onClick={() => pickLabelStyle(style.value)}
            >
              {style.name}
            </button>
          ))}
        </div>
      </div>
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
                    aria-pressed={!movie && each.number === season?.number}
                    className="h-full w-full transition-transform aria-pressed:-translate-y-3 motion-reduce:transition-none"
                    onClick={() => pickSeason(each.number)}
                  >
                    <Spine
                      name={selected.name}
                      caption={seasonLabel(each.number)}
                      logoFile={selected.logoFile}
                      spineColour={selected.spineColour}
                    />
                  </button>
                </li>
              ))}
              {selected.movies.map((each) => (
                <li key={each.movieId} className="h-72 w-12">
                  <button
                    type="button"
                    aria-label={each.title}
                    aria-pressed={each.movieId === movie?.movieId}
                    className="h-full w-full transition-transform aria-pressed:-translate-y-3 motion-reduce:transition-none"
                    onClick={() => setSelectedMovieId(each.movieId)}
                  >
                    <Spine name={each.title} logoFile={each.logoFile} spineColour={each.spineColour} />
                  </button>
                </li>
              ))}
            </ul>
            <div className="w-40">
              <Sleeve name={movie?.title ?? selected.name} posterFile={movie ? movie.posterFile : selected.posterFile} />
            </div>
            {hasTape && (
              <div className="w-80">
                <WallPosters
                  name={selected.name}
                  showPosterFile={selected.posterFile}
                  tapePosterFile={tapePosterFile}
                  tapeLabel={tapeLabel}
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
