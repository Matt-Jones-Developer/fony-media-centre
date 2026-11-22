export type SeasonArt = {
  number: number;
  name: string | null;
  overview: string | null;
  posterFile: string | null;
};

export type SeriesArt = {
  seriesId: number;
  name: string;
  overview: string | null;
  posterFile: string | null;
  logoFile: string | null;
  spineColour: string | null;
  seasons: SeasonArt[];
};
