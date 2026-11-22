"""The editable series map that lives at <library>/.tapehead/series.yaml."""

from __future__ import annotations

from dataclasses import asdict, dataclass, field
from pathlib import Path, PurePosixPath

import yaml

from .survey import Survey

HEADER = """\
# tapehead series map
# One entry per series found in this folder. Edit freely - `tapehead survey` keeps your edits.
#   mode:    tmdb  -> look up episode titles on TMDB (needs tmdb_id; `tapehead match` fills it in)
#            local -> no lookup; keep the numbers/titles found in the filenames
#            keep  -> include in the library as-is, but don't rename anything (curated folders)
#            skip  -> leave these files exactly where they are
#   force_season: put every file in this season (0 = Specials), e.g. stray segments from another show
#   name / year: what the series folder will be called, e.g. "Count Duckula (1988)"
#   tmdb_id: the number in a TMDB URL, e.g. themoviedb.org/tv/2504 -> 2504
"""


@dataclass
class SeriesEntry:
    key: str
    name: str
    year: int | None = None
    mode: str = "tmdb"
    tmdb_id: int | None = None
    tmdb_name: str | None = None
    force_season: int | None = None
    files: int = 0
    sample: str = ""
    patterns: dict = field(default_factory=dict)


def state_dir(root: Path) -> Path:
    d = root / ".tapehead"
    d.mkdir(exist_ok=True)
    return d


def load(root: Path) -> list[SeriesEntry]:
    f = root / ".tapehead" / "series.yaml"
    if not f.exists():
        return []
    data = yaml.safe_load(f.read_text()) or {}
    return [SeriesEntry(**{k: v for k, v in e.items() if k in SeriesEntry.__dataclass_fields__}) for e in data.get("series", [])]


def save(root: Path, entries: list[SeriesEntry]) -> Path:
    f = state_dir(root) / "series.yaml"
    body = yaml.safe_dump(
        {"series": [asdict(e) for e in entries]}, sort_keys=False, allow_unicode=True, width=120
    )
    f.write_text(HEADER + "\n" + body)
    return f


MARKER = ".tapehead-series.yaml"
MARKER_FIELDS = ("name", "year", "mode", "tmdb_id", "tmdb_name")


def read_marker(folder: Path) -> dict | None:
    f = folder / MARKER
    if not f.exists():
        return None
    try:
        return yaml.safe_load(f.read_text()) or None
    except yaml.YAMLError:
        return None


def write_marker(folder: Path, meta: dict) -> None:
    folder.mkdir(parents=True, exist_ok=True)
    body = {k: meta.get(k) for k in MARKER_FIELDS}
    (folder / MARKER).write_text(
        "# Written by tapehead after tidying this show. Lets future runs recognise it.\n"
        + yaml.safe_dump(body, sort_keys=False, allow_unicode=True)
    )


def merge(root: Path, sv: Survey) -> list[SeriesEntry]:
    """Combine a fresh survey with any edits already saved."""
    old = {e.key: e for e in load(root)}
    out = []
    for s in sv.series:
        prev = old.get(s.key)
        e = SeriesEntry(
            key=s.key,
            name=s.name,
            year=s.year,
            files=len(s.files),
            sample=PurePosixPath(s.files[0].path).name if s.files else "",
            patterns=dict(s.patterns),
        )
        marker = read_marker(root / s.top) if s.top and not prev else None
        if marker:
            for k in MARKER_FIELDS:
                if marker.get(k) is not None:
                    setattr(e, k, marker[k])
        if prev:
            e.name, e.year, e.mode, e.tmdb_id, e.tmdb_name, e.force_season = (
                prev.name, prev.year, prev.mode, prev.tmdb_id, prev.tmdb_name, prev.force_season
            )
        out.append(e)
    return out
