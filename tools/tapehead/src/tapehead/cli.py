"""tapehead - tidy up episode filenames for Movie Centre.

    tapehead survey  <folder>      read-only: what's in here, grouped by series
    tapehead match   <folder>      link each series to TMDB (asks when unsure)
    tapehead plan    <folder>      dry run: work out every rename, save a plan
    tapehead apply   <plan.json>   do it (logs every move)
    tapehead undo    <undo.jsonl>  put everything back
    tapehead clean   <folder>      bin torrent junk, subtitles and emptied folders
"""

from __future__ import annotations

import json
import os
import re
from collections import Counter
from pathlib import Path
from typing import Optional

import typer
from rapidfuzz import fuzz
from rich.console import Console
from rich.markup import escape
from rich.prompt import Prompt
from rich.table import Table

from . import state
from .apply import apply_plan, delete_clutter, empty_dirs, find_clutter, undo_log
from .plan import build_plan, save_plan
from .survey import survey as run_survey
from .tmdb import TMDB, TMDBError, save_key

app = typer.Typer(add_completion=False, no_args_is_help=True, help=__doc__)
con = Console()

SKIP_DIRS = {".tapehead", ".Trashes", ".Spotlight-V100", ".fseventsd", "_to_delete"}


def scan(root: Path) -> list[str]:
    out = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS and not d.startswith("._")]
        for fn in filenames:
            if fn.startswith("._") or fn == ".DS_Store":
                continue
            out.append(str(Path(dirpath, fn).relative_to(root)).replace(os.sep, "/"))
    return sorted(out)


def _root(folder: Path) -> Path:
    root = folder.expanduser().resolve()
    if not root.is_dir():
        con.print(f"[red]Not a folder:[/] {root}")
        raise typer.Exit(1)
    return root


def _survey(root: Path):
    with con.status("Scanning files…"):
        files = scan(root)
        sv = run_survey(files)
    return sv


@app.command()
def survey(folder: Path):
    """Read-only. Group files into series and write .tapehead/series.yaml for you to check."""
    root = _root(folder)
    sv = _survey(root)
    entries = state.merge(root, sv)
    path = state.save(root, entries)

    t = Table(title=f"{root.name}: {sv.total} files", show_lines=False)
    for col in ("Series", "Year", "Files", "Numbering", "Mode", "TMDB"):
        t.add_column(col)
    for e in entries:
        pats = ", ".join(f"{k}×{v}" for k, v in e.patterns.items())
        t.add_row(escape(e.name), str(e.year or "–"), str(e.files), pats, e.mode, str(e.tmdb_id or "–"))
    con.print(t)
    con.print(
        f"[bold]Also found:[/] {len(sv.movies)} movie(s), {len(sv.unknown)} unparsed, "
        f"{len(sv.dvd_dirs)} DVD folder(s), {len(sv.sidecars)} subtitle/sidecar files, {len(sv.junk)} junk files"
    )
    for label, items in (("Movies", [m.path for m in sv.movies]), ("Unparsed", [u.path for u in sv.unknown]), ("DVD folders", sv.dvd_dirs)):
        if items:
            con.print(f"\n[bold]{label}[/] (left untouched):")
            for i in items:
                con.print(f"  · {escape(i)}")
    con.print(f"\nSeries map saved to [cyan]{path}[/]. Next: [bold]tapehead match \"{folder}\"[/]")


def _norm(s: str) -> str:
    return re.sub(r"[^a-z0-9]", "", s.lower().replace("&", "and"))


def _score(e: state.SeriesEntry, r: dict) -> float:
    names = [r.get("name", ""), r.get("original_name", "")]
    s = max(fuzz.ratio(_norm(e.name), _norm(n)) for n in names if n)
    s = max(s, max(fuzz.ratio(_norm(re.sub(r"^the ", "", e.name, flags=re.I)), _norm(re.sub(r"^the ", "", n, flags=re.I))) for n in names if n))
    y = (r.get("first_air_date") or "")[:4]
    if e.year and y.isdigit():
        s += 8 if abs(int(y) - e.year) <= 1 else -12
    return s


@app.command()
def match(folder: Path, auto_only: bool = typer.Option(False, help="Don't ask; only take confident matches.")):
    """Link each series in series.yaml to a TMDB show."""
    root = _root(folder)
    entries = state.load(root)
    if not entries:
        con.print("[red]No series map yet - run `tapehead survey` first.[/]")
        raise typer.Exit(1)
    try:
        tmdb = TMDB()
    except TMDBError as err:
        con.print(f"[red]{err}[/]")
        raise typer.Exit(1)

    for e in entries:
        if e.mode != "tmdb" or e.tmdb_id:
            continue
        query = e.name
        while True:
            results = tmdb.search_tv(query, e.year)[:8]
            ranked = sorted(results, key=lambda r: -_score(e, r))
            best = ranked[0] if ranked else None
            gap = _score(e, ranked[0]) - _score(e, ranked[1]) if len(ranked) > 1 else 99
            if best and _score(e, best) >= 95 and gap >= 8:
                _pick(e, best)
                con.print(f"[green]✓[/] {e.name} → {e.tmdb_name} (tmdb {e.tmdb_id})")
                break
            if auto_only:
                con.print(f"[yellow]?[/] {e.name}: not sure, left for you")
                break
            con.print(f"\n[bold]{escape(e.name)}[/] ({e.year or 'year?'}) - {e.files} files, e.g. [dim]{escape(e.sample)}[/]")
            t = Table(show_header=True)
            for col in ("#", "Title", "First aired", "Country", "Overview"):
                t.add_column(col, overflow="fold")
            for i, r in enumerate(ranked[:6], 1):
                t.add_row(str(i), r.get("name", ""), (r.get("first_air_date") or "")[:4], ",".join(r.get("origin_country", [])), (r.get("overview") or "")[:90])
            con.print(t if ranked else "[yellow]No results.[/]")
            ans = Prompt.ask("Pick a number, [b]s[/]kip, [b]l[/]ocal (no lookup), id:12345, or type a new search", default="1" if ranked else "s")
            if ans.isdigit() and 1 <= int(ans) <= len(ranked[:6]):
                _pick(e, ranked[int(ans) - 1])
                break
            if ans.lower().startswith("id:"):
                show = tmdb.tv(int(ans[3:]))
                _pick(e, show)
                break
            if ans.lower() == "s":
                break
            if ans.lower() == "l":
                e.mode = "local"
                break
            query = ans
        state.save(root, entries)
    con.print(f"\nSaved. Next: [bold]tapehead plan \"{folder}\"[/]")


def _pick(e: state.SeriesEntry, r: dict) -> None:
    e.tmdb_id = r["id"]
    e.tmdb_name = r.get("name")
    y = (r.get("first_air_date") or "")[:4]
    if y.isdigit():
        e.year = int(y)


def _make_plan(root: Path, dest: Optional[Path] = None):
    """Build and save a plan. TMDB data is kept in <root>/.tapehead/tmdb so plans can be re-made offline."""
    entries = state.load(root)
    if not entries:
        con.print("[red]Run `tapehead survey` first.[/]")
        raise typer.Exit(1)
    sv = _survey(root)
    tmdb = None
    try:
        tmdb = TMDB()
    except TMDBError:
        pass
    store = state.state_dir(root) / "tmdb"
    store.mkdir(exist_ok=True)
    movies_file = store / "movies.json"
    movie_cache = json.loads(movies_file.read_text()) if movies_file.exists() else {}

    def movie_lookup(title: str, year):
        k = f"{title}|{year}"
        if k in movie_cache:
            return tuple(movie_cache[k]) if movie_cache[k] else None
        if tmdb is None:
            return None
        hit = None
        try:
            for r in tmdb.search_movie(title, year)[:5]:
                if fuzz.ratio(_norm(title), _norm(r.get("title", ""))) >= 80:
                    y = (r.get("release_date") or "")[:4]
                    hit = (r["title"], int(y) if y.isdigit() else year)
                    break
        except TMDBError:
            return None
        movie_cache[k] = hit
        movies_file.write_text(json.dumps(movie_cache, indent=1))
        return hit

    def episodes_for(tv_id: int) -> list:
        f = store / f"tv-{tv_id}.json"
        if f.exists() and (tmdb is None or f.stat().st_mtime > __import__("time").time() - 30 * 86400):
            return json.loads(f.read_text())
        if tmdb is None:
            con.print(f"[red]No TMDB key, and no saved episode list for TMDB id {tv_id}. Run `tapehead config --tmdb-key ...`[/]")
            raise typer.Exit(1)
        with con.status(f"Fetching episode list for TMDB {tv_id}…"):
            eps = tmdb.all_episodes(tv_id)
        f.write_text(json.dumps(eps))
        return eps

    p = build_plan(root, sv, entries, episodes_for, dest.expanduser().resolve() if dest else None, movie_lookup)
    jf, cf = save_plan(root, p)
    return p, jf, cf


@app.command()
def plan(folder: Path, dest: Optional[Path] = typer.Option(None, help="Build the tidy library somewhere else on the same drive.")):
    """Dry run. Works out every rename and saves a plan (JSON + CSV). Nothing is moved."""
    root = _root(folder)
    p, jf, cf = _make_plan(root, dest)

    t = Table(title="Plan (nothing moved yet)")
    for col in ("Series", "Files", "High", "Medium", "Low", "No match", "Dupes"):
        t.add_column(col)
    for s in p["series"]:
        t.add_row(s["series"], str(s["files"]), str(s.get("high", 0)), str(s.get("medium", 0)), str(s.get("low", 0)),
                  str(s.get("none", 0) + s.get("unmatched_series", 0)), str(s.get("duplicate", 0)))
    con.print(t)

    by_series: dict[str, list] = {}
    for m in p["moves"]:
        by_series.setdefault(m["series"], []).append(m)
    for name, ms in by_series.items():
        con.print(f"\n[bold]{escape(name)}[/]")
        for m in ms[:3]:
            con.print(f"  [dim]{escape(Path(m['from']).name)}[/]\n   → {escape(m['to'])}  [{_col(m['confidence'])}]{m['confidence']}[/]")
        odd = [m for m in ms if m["confidence"] != "high"]
        for m in odd[:5]:
            con.print(f"  [{_col(m['confidence'])}]{m['confidence']}[/] {escape(Path(m['from']).name)} → {escape(Path(m['to']).name)}\n      [dim]{escape(m['reason'])}[/]")
        if len(odd) > 5:
            con.print(f"  …and {len(odd) - 5} more needing a look (see CSV)")

    levels = Counter(m["confidence"] for m in p["moves"])
    con.print(
        f"\n{len(p['moves'])} renames planned: {levels.get('high', 0)} high, {levels.get('medium', 0)} medium, "
        f"{levels.get('low', 0)} low. {len(p['skipped'])} files left alone."
    )
    con.print(f"Plan: [cyan]{jf}[/]\nOpen in Numbers/Excel: [cyan]{cf}[/]")
    con.print(f"When happy: [bold]tapehead apply \"{jf}\"[/]  (high confidence only by default)")


def _col(level: str) -> str:
    return {"high": "green", "medium": "yellow", "low": "red"}.get(level, "white")


@app.command()
def apply(
    plan_file: Path,
    include: str = typer.Option("high", help="Lowest confidence to apply: high, medium or low."),
    yes: bool = typer.Option(False, "--yes", "-y", help="Don't ask for confirmation."),
    root: Optional[Path] = typer.Option(None, help="Library folder, if the drive is mounted somewhere else now."),
):
    """Do the renames in a plan. Every move is logged so `tapehead undo` can reverse it."""
    p = json.loads(plan_file.read_text())
    levels = {"high": ["high"], "medium": ["high", "medium"], "low": ["high", "medium", "low"]}[include]
    n = sum(1 for m in p["moves"] if m["confidence"] in levels)
    if not yes and not typer.confirm(f"Move {n} files inside {p['root']}?"):
        raise typer.Exit()
    undo, stats = apply_plan(p, include, log=lambda s: con.print(escape(s)), root=root)
    con.print(f"[green]Done.[/] {stats}")
    con.print(f"Undo with: [bold]tapehead undo \"{undo}\"[/]")
    con.print("Tidy up leftovers with: [bold]tapehead clean <folder>[/]")


@app.command()
def undo(log_file: Path, root: Optional[Path] = typer.Option(None, help="Library folder, if mounted elsewhere.")):
    """Reverse an `apply`, using its undo log."""
    stats = undo_log(log_file, log=lambda s: con.print(escape(s)), root=root)
    con.print(f"[green]Undone.[/] {stats}")


@app.command()
def run(
    folder: Path,
    apply_: bool = typer.Option(False, "--apply", help="Apply high-confidence renames."),
    clean_: bool = typer.Option(False, "--clean", help="Also delete junk/subtitles/emptied folders."),
):
    """Everything in one go, for new additions: survey → auto-match → plan → (apply) → (clean).

    Anything tapehead isn't sure about is left where it is and listed, so it's safe to run
    on a schedule. Run `tapehead match` by hand now and then to sort out new shows it couldn't place.
    """
    root = _root(folder)
    sv = _survey(root)
    entries = state.merge(root, sv)
    state.save(root, entries)
    new = [e for e in entries if e.mode == "tmdb" and not e.tmdb_id]
    if new:
        try:
            match(folder, auto_only=True)
        except typer.Exit:
            pass
    p, jf, cf = _make_plan(root)
    levels = Counter(m["confidence"] for m in p["moves"])
    con.print(f"Plan: {len(p['moves'])} renames ({levels.get('high', 0)} high, {levels.get('medium', 0)} medium, {levels.get('low', 0)} low) → {jf.name}")
    waiting = [e.name for e in state.load(root) if e.mode == "tmdb" and not e.tmdb_id]
    if waiting:
        con.print(f"[yellow]Needs you (`tapehead match`):[/] {', '.join(escape(w) for w in waiting)}")
    if apply_ and levels.get("high"):
        undo, stats = apply_plan(p, "high", log=lambda s: con.print(escape(s)))
        con.print(f"[green]Applied.[/] {stats}  undo: {undo.name}")
    if clean_:
        files, folders = find_clutter(root)
        con.print(f"[green]Cleaned.[/] {delete_clutter(files, folders)}")


@app.command()
def clean(
    folder: Path,
    keep_subs: bool = typer.Option(False, help="Keep subtitle files (.srt/.sub/.idx)."),
    really: bool = typer.Option(False, "--really", help="Actually delete. Without this it's a dry run."),
):
    """Bin torrent junk (.txt/.nfo/.db), sample clips, subtitles, and folders left with no video."""
    root = _root(folder)
    files, folders = find_clutter(root, keep_subs=keep_subs)
    for d in folders:
        inside = sum(1 for _ in d.rglob("*"))
        con.print(f"  [red]folder[/] {escape(str(d.relative_to(root)))}  [dim]({inside} items, no video)[/]")
    by_ext = Counter(f.suffix.lower() or f.name for f in files)
    con.print(f"  [red]files[/]  {len(files)}: " + ", ".join(f"{k}×{v}" for k, v in by_ext.most_common()))
    if not really:
        con.print("\nDry run - nothing deleted. Add [bold]--really[/] to delete.")
        return
    if not typer.confirm(f"Permanently delete {len(files)} files and {len(folders)} folders?"):
        raise typer.Exit()
    con.print(f"[green]Done.[/] {delete_clutter(files, folders)}")


@app.command()
def config(tmdb_key: str = typer.Option(..., help="TMDB v3 API key or v4 read access token.")):
    """Save your TMDB key to ~/.config/tapehead/config.toml."""
    path = save_key(tmdb_key)
    con.print(f"Saved to {path}")


if __name__ == "__main__":
    app()
