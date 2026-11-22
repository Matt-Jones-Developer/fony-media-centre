import { convertFileSrc } from "@tauri-apps/api/core";

interface SleeveProps {
  name: string;
  posterFile: string | null;
}

/** The front of the tape case. Sized by its parent's width. Without a poster it gets a handwritten label. */
export default function Sleeve({ name, posterFile }: SleeveProps) {
  return (
    <div className="w-full rounded-sm bg-plastic-dark p-2 shadow-lg shadow-black/60">
      <SleeveFront name={name} posterFile={posterFile} />
    </div>
  );
}

function SleeveFront({ name, posterFile }: SleeveProps) {
  if (posterFile) {
    return <img src={convertFileSrc(posterFile)} alt={`${name} case`} className="aspect-2/3 w-full object-cover" />;
  }
  return (
    <div className="grid aspect-2/3 w-full place-items-center bg-label-cream p-3 text-center font-hand text-3xl font-bold text-dymo-black graf:font-graf graf:text-5xl graf:leading-none graf:font-normal">
      {name}
    </div>
  );
}
