"""Build a rename plan (JSON) from a survey + series map + TMDB episode lists."""

from __future__ import annotations

import csv
import json
import re
from collections import Counter, defaultdict
from datetime import datetime
from pathlib import Path, PurePosixPath

from .episodes import EpisodeMatch, match_episode
from .parse import Parsed, smart_title
from .state import SeriesEntry, state_dir
from .survey import Survey

ILLEGAL = re.compile(r'[/:\\*?"<>|]')


def safe(text: str) -> str:
    text = ILLEGAL.sub(lambda m: {":": " -", "/": "-", "?": "", '"': "'"}.get(m.group(0), ""), text)
    return re.sub(r"\s+", " ", text).strip(" .")


def _norm(t: str) -> str:
    return re.sub(r"[^a-z0-9]", "", t.lower())


def series_folder(e: SeriesEntry) -> str:
    name = e.tmdb_name or e.name
    return safe(f"{name} ({e.year})" if e.year else name)


def episode_filename(show: str, m: EpisodeMatch, ext: str) -> str:
    code = f"S{m.season:02}E{m.episode:02}"
    if m.episode_end:
        code += f"-E{m.episode_end:02}"
    if m.segment:
        code += m.segment
    title = f" - {m.title}" if m.title else ""
    return safe(f"{show} - {code}{title}") + ext


def season_folder(season: int) -> str:
    return "Specials" if season == 0 else f"Season {season:02}"


def _local_match(f: Parsed) -> EpisodeMatch:
    season = f.season if f.season is not None else 1
    episode = f.episode if f.episode is not None else f.absolute
    title = f.title_hint
    if title and (title.islower() or title.isupper()):
        title = smart_title(title)
    if episode is None:
        return EpisodeMatch(None, None, None, None, "", "none", "no number in filename")
    return EpisodeMatch(season, episode, f.episode_end, f.segment, title, "high", "local (filename numbering)", "local")


def _sidecars_for(video_rel: str, sidecars_by_dir: dict[str, list[str]]) -> list[tuple[str, str]]:
    """Return (sidecar_rel, suffix) pairs, e.g. ('x/ep.en.srt', '.en.srt')."""
    p = PurePosixPath(video_rel)
    out = []
    dirs = [str(p.parent)] + [str(p.parent / sub) for sub in ("Subs", "subs", "Subtitles", "subtitles", "Sub")]
    for s in (x for d in dirs for x in sidecars_by_dir.get(d, [])):
        name = PurePosixPath(s).name
        if name.startswith(p.stem + ".") or name.startswith(p.stem + " "):
            out.append((s, name[len(p.stem):]))
    return out


def build_plan(root: Path, sv: Survey, entries: list[SeriesEntry], episodes_for, dest: Path | None = None, movie_lookup=None) -> dict:
    """episodes_for(tmdb_id) -> list of TMDB episode dicts.
    movie_lookup(title, year) -> (title, year) or None."""
    dest = dest or root
    by_key = {s.key: s for s in sv.series}
    sidecars_by_dir: dict[str, list[str]] = defaultdict(list)
    for sc in sv.sidecars:
        sidecars_by_dir[str(PurePosixPath(sc.path).parent)].append(sc.path)

    moves, skipped, series_summary = [], [], []
    taken: dict[str, str] = {}

    for e in entries:
        s = by_key.get(e.key)
        if not s:
            continue
        counts: Counter = Counter()
        if e.mode in ("skip", "keep"):
            why = "series set to skip" if e.mode == "skip" else "kept as-is (curated folder)"
            for f in s.files:
                skipped.append({"path": f.path, "reason": why})
            series_summary.append({"series": e.name, "files": len(s.files), e.mode: len(s.files)})
            continue
        if e.mode == "tmdb" and not e.tmdb_id:
            for f in s.files:
                skipped.append({"path": f.path, "reason": "series has no tmdb_id yet (run `tapehead match`)"})
            series_summary.append({"series": e.name, "files": len(s.files), "unmatched_series": len(s.files)})
            continue

        eps = episodes_for(e.tmdb_id) if e.mode == "tmdb" else []
        season_sizes = Counter(ep["season"] for ep in eps)
        file_max = defaultdict(int)
        for f in s.files:
            if f.season is not None and f.episode is not None:
                file_max[f.season] = max(file_max[f.season], f.episode)
        overfull = {sn for sn, mx in file_max.items() if eps and mx > season_sizes.get(sn, 0)}

        show = series_folder(e)
        matched = []
        for f in sorted(s.files, key=lambda x: x.path):
            m = match_episode(f, eps, e.tmdb_name or e.name) if e.mode == "tmdb" else _local_match(f)
            if e.force_season is not None and m.season is not None:
                m.season = e.force_season
                m.reason += f"; forced into {season_folder(e.force_season)}"
            if m.confidence == "high" and m.reason.startswith("number only") and f.season in overfull:
                m.confidence = "low"
                m.reason = (
                    f"filename numbering goes to E{file_max[f.season]} but TMDB season {f.season} has "
                    f"{season_sizes.get(f.season, 0)} episodes - probably numbered by segment"
                )
            matched.append((f, m))

        # Does this series' own numbering follow TMDB's? Judge from files whose titles matched strongly.
        if e.mode == "tmdb":
            agree = sum(1 for f, m in matched if m.source == "both")
            differ = sum(
                1 for f, m in matched
                if m.source in ("title", "range") and m.confidence == "high"
                and f.episode is not None and (m.season, m.episode) != (f.season, f.episode)
            )
            if differ > agree:
                for f, m in matched:
                    if m.source == "number" and m.confidence in ("high", "medium"):
                        m.confidence = "low"
                        m.reason += f"; this series' file numbering doesn't follow TMDB ({differ} titles disagreed, {agree} agreed)"

        # Number-only matches (no title in the filename to check against) are only trusted when the
        # numbering is verified: the season has exactly as many files as TMDB has episodes, or this
        # series' titled files overwhelmingly agree with TMDB's numbering.
        if e.mode == "tmdb":
            files_per_season = Counter(f.season for f in s.files if f.season is not None and f.episode is not None)
            max_per_season = defaultdict(int)
            for f in s.files:
                if f.season is not None and f.episode is not None:
                    max_per_season[f.season] = max(max_per_season[f.season], f.episode)
            agree_all = sum(1 for _, m in matched if m.source == "both")
            differ_all = sum(1 for f, m in matched if m.source in ("title", "range") and f.episode is not None
                             and (m.season, m.episode) != (f.season, f.episode))
            series_ok = agree_all >= 5 and agree_all >= 3 * differ_all
            for f, m in matched:
                if m.source != "number" or m.confidence != "high" or not m.reason.startswith("number only"):
                    continue
                full = files_per_season[f.season] == season_sizes.get(f.season, -1) == max_per_season[f.season]
                if not (series_ok or full):
                    m.confidence = "low"
                    m.reason = (f"number only and can't verify numbering: {files_per_season[f.season]} files in "
                                f"season {f.season}, TMDB has {season_sizes.get(f.season, 0)} - leaving as-is")

        # Hand out episodes best-first, so a strong title match beats a guess from numbering.
        rank = {"high": 0, "medium": 1, "low": 2, "none": 3}
        src_rank = {"both": 0, "range": 1, "title": 2, "local": 3, "number": 4, "none": 5}
        claimed: dict[tuple, str] = {}
        for f, m in sorted(matched, key=lambda fm: (rank[fm[1].confidence], src_rank.get(fm[1].source, 9), -fm[1].score, fm[0].path)):
            if m.season is None:
                counts[m.confidence] += 1
                skipped.append({"path": f.path, "reason": m.reason})
                continue
            codes = [(m.season, n, m.segment) for n in range(m.episode, (m.episode_end or m.episode) + 1)]
            clash = next((claimed[c] for c in codes if c in claimed), None)
            if clash:
                if m.source in ("title", "both", "range") and m.confidence == "high":
                    counts["duplicate"] += 1
                    skipped.append({"path": f.path, "reason": f"duplicate of {clash}"})
                else:
                    counts["low"] += 1
                    skipped.append({"path": f.path, "reason": f"{m.reason}; that episode already belongs to {clash}"})
                continue
            for c in codes:
                claimed[c] = f.path
            counts[m.confidence] += 1
            ext = PurePosixPath(f.path).suffix.lower()
            new_name = episode_filename(show, m, ext)
            target = str(PurePosixPath(show) / season_folder(m.season) / new_name)
            target_abs = str(dest / target) if dest != root else target
            taken[target_abs] = f.path
            move = {
                "from": f.path,
                "to": target_abs,
                "confidence": m.confidence,
                "reason": m.reason,
                "series": e.name,
                "series_key": e.key,
                "sidecars": [
                    {"from": sc, "to": str(PurePosixPath(target_abs).with_name(PurePosixPath(new_name).stem + suf))}
                    for sc, suf in _sidecars_for(f.path, sidecars_by_dir)
                ],
            }
            if move["from"] != move["to"]:
                moves.append(move)
        series_summary.append({"series": e.name, "files": len(s.files), **counts})

    # movies and extras sit inside the main series of the top-level folder they were found in
    main_for_top: dict[str, SeriesEntry] = {}
    for e in sorted(entries, key=lambda x: -x.files):
        top = e.key.split("::", 1)[0]
        if e.mode not in ("skip",) and top not in main_for_top:
            main_for_top[top] = e

    def owner(f: Parsed) -> SeriesEntry | None:
        parts = PurePosixPath(f.path).parts
        return main_for_top.get(parts[0]) if len(parts) > 1 else None

    for f in sv.movies:
        e = owner(f)
        if not e or e.mode == "keep":
            skipped.append({"path": f.path, "reason": "movie with no series folder to sit in"})
            continue
        show_name = e.tmdb_name or e.name
        title, year = f.title_hint, f.aired_year
        if _norm(show_name.split()[0]) not in _norm(title) and not _norm(title).startswith(_norm(show_name)):
            title = f"{show_name} {title}"            # "The Movie" -> "Hey Arnold! The Movie"
        confidence, reason = "high", "movie (named from filename)"
        if movie_lookup:
            hit = movie_lookup(title, year)
            if hit:
                title = hit[0]
                year = year or hit[1]          # the year in your filename wins over TMDB's
                reason = "movie (TMDB)"
        name = safe(f"{title} ({year})" if year else title) + PurePosixPath(f.path).suffix.lower()
        target = str(PurePosixPath(series_folder(e)) / "Movies" / name)
        if target not in taken and target != f.path:
            taken[target] = f.path
            moves.append({"from": f.path, "to": target, "confidence": confidence, "reason": reason, "series": e.name, "sidecars": []})

    for f in sv.extras:
        e = owner(f)
        if not e or e.mode == "keep":
            skipped.append({"path": f.path, "reason": "extra with no series folder to sit in"})
            continue
        name = safe(f.title_hint or PurePosixPath(f.path).stem) + PurePosixPath(f.path).suffix.lower()
        target = str(PurePosixPath(series_folder(e)) / "Extras" / name)
        if target not in taken and target != f.path:
            taken[target] = f.path
            moves.append({"from": f.path, "to": target, "confidence": "high", "reason": "extra / featurette", "series": e.name, "sidecars": []})

    for u in sv.unknown:
        skipped.append({"path": u.path, "reason": "couldn't parse an episode number"})
    for d in sv.dvd_dirs:
        skipped.append({"path": d, "reason": "DVD rip (VIDEO_TS) - leave as is, or remux to MKV"})

    moves.sort(key=lambda m: m["to"])
    return {
        "root": str(root),
        "dest": str(dest),
        "created": datetime.now().isoformat(timespec="seconds"),
        "moves": moves,
        "skipped": skipped,
        "series": series_summary,
        "series_meta": {
            e.key: {"name": e.name, "year": e.year, "mode": e.mode, "tmdb_id": e.tmdb_id, "tmdb_name": e.tmdb_name}
            for e in entries
        },
    }


def save_plan(root: Path, plan: dict) -> tuple[Path, Path]:
    stamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    d = state_dir(root)
    jf = d / f"plan-{stamp}.json"
    jf.write_text(json.dumps(plan, indent=2, ensure_ascii=False))
    cf = d / f"plan-{stamp}.csv"
    with cf.open("w", newline="", encoding="utf-8") as fh:
        w = csv.writer(fh)
        w.writerow(["confidence", "series", "from", "to", "reason"])
        for m in plan["moves"]:
            w.writerow([m["confidence"], m["series"], m["from"], m["to"], m["reason"]])
        for s in plan["skipped"]:
            w.writerow(["skipped", "", s["path"], "", s["reason"]])
    return jf, cf
