/** The caption under a season's spine label. 0 is Specials; Looney Tunes seasons are decades, numbered by their first year. */
export function seasonLabel(seasonNumber: number) {
  if (seasonNumber === 0) return "SP";
  if (seasonNumber >= 1900) return `${seasonNumber}s`;
  return String(seasonNumber);
}
