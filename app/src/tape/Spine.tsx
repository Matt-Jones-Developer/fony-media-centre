import type { CSSProperties } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";

interface SpineProps {
  name: string;
  seasonNumber: number;
  logoFile: string | null;
  spineColour: string | null;
}

/** A tape seen spine-out. Sized by its parent. Without a logo and colour it gets a handwritten label. */
export default function Spine({ name, seasonNumber, logoFile, spineColour }: SpineProps) {
  return (
    <div className="spine-ribs flex h-full w-full flex-col rounded-sm p-1 shadow-lg shadow-black/60">
      <SpineLabel name={name} label={seasonLabel(seasonNumber)} logoFile={logoFile} spineColour={spineColour} />
    </div>
  );
}

interface SpineLabelProps {
  name: string;
  label: string;
  logoFile: string | null;
  spineColour: string | null;
}

function SpineLabel({ name, label, logoFile, spineColour }: SpineLabelProps) {
  if (logoFile && spineColour) {
    return <PrintedLabel name={name} label={label} logoFile={logoFile} spineColour={spineColour} />;
  }
  return <HandwrittenLabel name={name} label={label} />;
}

interface PrintedLabelProps {
  name: string;
  label: string;
  logoFile: string;
  spineColour: string;
}

function PrintedLabel({ name, label, logoFile, spineColour }: PrintedLabelProps) {
  const textColour = isLightColour(spineColour) ? "text-dymo-black" : "text-paper";
  // The colour comes from the poster at runtime, so it can't be a Tailwind token. Cast: React's type has no custom properties.
  const colourVariable = { "--spine-colour": spineColour } as CSSProperties;

  return (
    <div style={colourVariable} className="flex flex-1 flex-col rounded-xs bg-(--spine-colour) py-2">
      {/* Size container: the logo is laid out sideways, so its width has to come from the strip's height. */}
      <div className="relative flex-1 overflow-hidden [container-type:size]">
        <img
          src={convertFileSrc(logoFile)}
          alt={name}
          className="absolute top-1/2 left-1/2 h-[100cqw] w-[100cqh] max-w-none -translate-1/2 -rotate-90 object-contain p-1"
        />
      </div>
      <p className={`pt-2 text-center font-mono text-sm font-bold ${textColour}`}>{label}</p>
    </div>
  );
}

interface HandwrittenLabelProps {
  name: string;
  label: string;
}

function HandwrittenLabel({ name, label }: HandwrittenLabelProps) {
  return (
    <div className="flex flex-1 flex-col items-center rounded-xs bg-label-cream py-2 text-dymo-black">
      <p className="flex-1 rotate-180 overflow-hidden text-center font-hand text-sm leading-tight [writing-mode:vertical-rl]">
        {name}
      </p>
      <p className="pt-2 font-hand text-sm">{label}</p>
    </div>
  );
}

/** 0 is Specials; Looney Tunes seasons are decades, numbered by their first year. */
function seasonLabel(seasonNumber: number) {
  if (seasonNumber === 0) return "SP";
  if (seasonNumber >= 1900) return `${seasonNumber}s`;
  return String(seasonNumber);
}

/** True when dark text reads better than light text on `hex` (#rrggbb). */
function isLightColour(hex: string) {
  const red = parseInt(hex.slice(1, 3), 16);
  const green = parseInt(hex.slice(3, 5), 16);
  const blue = parseInt(hex.slice(5, 7), 16);
  const brightness = (red * 299 + green * 587 + blue * 114) / 1000;
  return brightness > 150;
}
