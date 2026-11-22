"""Group parsed files into series so each one can be matched to TMDB once."""

from __future__ import annotations

import re
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from pathlib import PurePosixPath

from .parse import (
    NEN_RE,
    NXN_RE,
    SXE_RE,
    Parsed,
    classify_ext,
    clean_show_name,
    clean_spaces,
    common_token_prefix,
    is_season_dir,
    parse_path,
)


def norm(name: str) -> str:
    n = name.lower().replace("&", "and")
    n = re.sub(r"\bn'|'n'|\bn\b", "and", n)
    n = re.sub(r"^the\s+", "", n)
    n = re.sub(r"[^a-z0-9]+", "", n)
    return n


@dataclass
class Series:
    key: str
    name: str
    year: int | None
    top: str
    files: list[Parsed] = field(default_factory=list)

    @property
    def patterns(self) -> Counter:
        return Counter(f.pattern for f in self.files)


@dataclass
class Survey:
    series: list[Series]
    movies: list[Parsed]
    extras: list[Parsed]
    dvd_dirs: list[str]
    unknown: list[Parsed]
    sidecars: list[Parsed]
    junk: list[Parsed]
    total: int


def _is_release_dir(d: str) -> bool:
    c = clean_spaces(d)
    return bool(SXE_RE.search(c) and not re.search(r"\bS\d{1,2}\s*-\s*S\d{1,2}\b", c, re.I))


def _folder_name(rel: str) -> tuple[str, int | None]:
    dirs = list(PurePosixPath(rel).parts[:-1])
    for d in reversed(dirs):
        if is_season_dir(d) or _is_release_dir(d) or d.upper() == "VIDEO_TS":
            continue
        name, year = clean_show_name(d)
        if name:
            return name, year
    return "", None


def survey(rel_paths: list[str]) -> Survey:
    # 1. common word-prefix per directory, for files without an explicit SxxEyy
    by_dir: dict[str, list[str]] = defaultdict(list)
    for rel in rel_paths:
        p = PurePosixPath(rel)
        if classify_ext(p.suffix.lower()) == "video":
            by_dir[str(p.parent)].append(p.stem)
    prefixes = {}
    for d, stems in by_dir.items():
        explicit = [s for s in stems if any(rx.search(clean_spaces(s)) for rx in (SXE_RE, NXN_RE, NEN_RE))]
        prefixes[d] = "" if len(explicit) > len(stems) / 2 else common_token_prefix(stems)

    parsed = [parse_path(rel, prefixes.get(str(PurePosixPath(rel).parent), "")) for rel in rel_paths]

    movies, extras, unknown, sidecars, junk = [], [], [], [], []
    dvd_dirs: set[str] = set()
    episodes: list[Parsed] = []
    for f in parsed:
        if f.kind == "episode":
            episodes.append(f)
        elif f.kind == "movie":
            movies.append(f)
        elif f.kind == "extra":
            extras.append(f)
        elif f.kind == "dvd":
            parts = PurePosixPath(f.path).parts
            idx = [i for i, x in enumerate(parts) if x.upper() == "VIDEO_TS"]
            dvd_dirs.add(str(PurePosixPath(*parts[: idx[0]])) if idx else str(PurePosixPath(f.path).parent))
        elif f.kind == "sidecar":
            sidecars.append(f)
        elif f.kind == "unknown":
            unknown.append(f)
        else:
            junk.append(f)

    # 2. files whose filename gave no show name inherit the dominant name in their top folder
    def top_of(rel: str) -> str:
        parts = PurePosixPath(rel).parts
        return parts[0] if len(parts) > 1 else ""

    dominant: dict[str, tuple[str, int | None]] = {}
    names_by_top: dict[str, Counter] = defaultdict(Counter)
    for f in episodes:
        if f.name_hint:
            names_by_top[top_of(f.path)][(f.name_hint, f.year_hint)] += 1
    for top, c in names_by_top.items():
        dominant[top] = c.most_common(1)[0][0]

    groups: dict[tuple[str, str], Series] = {}
    for f in episodes:
        top = top_of(f.path)
        name, year = f.name_hint, f.year_hint
        if not name:
            fname, fyear = _folder_name(f.path)
            dname, dyear = dominant.get(top, ("", None))
            # prefer the dominant filename name if it's the same show as the folder
            if dname and (not fname or norm(dname) in norm(fname) or norm(fname) in norm(dname)):
                name, year = dname, dyear or fyear
            else:
                name, year = fname, fyear
            f.name_hint = name
        key = (top, norm(name) + (f"-{year}" if year and _year_in_name(f) else "") + _subfolder_year(f.path))
        if key not in groups:
            groups[key] = Series(key=f"{top}::{key[1]}", name=name, year=year, top=top)
        g = groups[key]
        g.files.append(f)
        if g.year is None and year:
            g.year = year

    # 3. pick a better display name / year from the top folder when it is a longer form of the same name
    for g in groups.values():
        tname, tyear = clean_show_name(g.top) if g.top else ("", None)
        if tname and norm(tname).startswith(norm(g.name)) and len(norm(tname)) > len(norm(g.name)) and len(groups_for_top(groups, g.top)) == 1:
            g.name = tname
        if g.year is None and len(groups_for_top(groups, g.top)) == 1:
            g.year = tyear
        if g.year is None:
            years = Counter()
            for f in g.files:
                for d in PurePosixPath(f.path).parts[:-1]:
                    _, y = clean_show_name(d)
                    if y:
                        years[y] += 1
            if years:
                g.year = years.most_common(1)[0][0]

    series = sorted(groups.values(), key=lambda s: (s.top.lower(), s.name.lower()))
    return Survey(series, movies, extras, sorted(dvd_dirs), unknown, sidecars, junk, len(rel_paths))


def _subfolder_year(rel: str) -> str:
    """A year on a non-season sub-folder splits same-named series (Hulk 1982 vs 1996)."""
    for d in PurePosixPath(rel).parts[1:-1]:
        if is_season_dir(d) or _is_release_dir(d):
            continue
        _, y = clean_show_name(d)
        if y:
            return f"@{y}"
    return ""


def _year_in_name(f: Parsed) -> bool:
    return f.year_hint is not None


def groups_for_top(groups: dict, top: str) -> list:
    return [g for (t, _), g in groups.items() if t == top]
