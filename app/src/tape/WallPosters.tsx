import { convertFileSrc } from "@tauri-apps/api/core";

interface WallPostersProps {
  name: string;
  seasonNumber: number;
  showPosterFile: string | null;
  seasonPosterFile: string | null;
}

/** The two 2:3 posters either side of the TV: the show on the left, the season on the right. */
export default function WallPosters({ name, seasonNumber, showPosterFile, seasonPosterFile }: WallPostersProps) {
  return (
    <div className="grid grid-cols-2 gap-4">
      <WallPoster file={showPosterFile} label={`${name} poster`} />
      <WallPoster file={seasonPosterFile ?? showPosterFile} label={`${name} season ${seasonNumber} poster`} />
    </div>
  );
}

interface WallPosterProps {
  file: string | null;
  label: string;
}

function WallPoster({ file, label }: WallPosterProps) {
  if (!file) {
    return <div aria-label={`${label} (none)`} className="aspect-2/3 w-full bg-wall-dark" />;
  }
  return <img src={convertFileSrc(file)} alt={label} className="aspect-2/3 w-full object-cover shadow-lg shadow-black/60" />;
}
