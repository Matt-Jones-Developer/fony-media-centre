"""Carry out a plan, logging every move so it can be undone. Also: clean up junk."""

from __future__ import annotations

import json
import os
import shutil
from datetime import datetime
from pathlib import Path

from .parse import JUNK_EXTS, SIDECAR_EXTS, VIDEO_EXTS
from collections import Counter

from .state import state_dir, write_marker

LEVELS = {"high": 0, "medium": 1, "low": 2}
KEEP_EXTS = VIDEO_EXTS | {".vob", ".ifo", ".bup"}


def _move(src: Path, dst: Path) -> None:
    dst.parent.mkdir(parents=True, exist_ok=True)
    try:
        os.rename(src, dst)          # instant on the same drive
    except OSError:
        shutil.move(str(src), str(dst))


def apply_plan(plan: dict, include: str = "high", log=print, root: Path | None = None) -> tuple[Path, dict]:
    """Paths in the plan and the undo log are relative to the library root, so a plan made
    on the Mac can be applied anywhere the drive is mounted (pass `root` to override)."""
    root = Path(root or plan["root"])
    limit = LEVELS[include]
    stamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    undo = state_dir(root) / f"undo-{stamp}.jsonl"
    stats = {"moved": 0, "sidecars": 0, "skipped_level": 0, "missing": 0, "exists": 0}
    landed: dict[str, Counter] = {}

    with undo.open("a", encoding="utf-8") as fh:
        for m in plan["moves"]:
            if LEVELS.get(m["confidence"], 9) > limit:
                stats["skipped_level"] += 1
                continue
            pairs = [(m["from"], m["to"])] + [(s["from"], s["to"]) for s in m.get("sidecars", [])]
            for i, (a, b) in enumerate(pairs):
                src, dst = root / a, root / b
                if not src.exists():
                    stats["missing"] += 1
                    log(f"missing: {a}")
                    continue
                if dst.exists():
                    stats["exists"] += 1
                    log(f"already exists, left alone: {b}")
                    continue
                _move(src, dst)
                fh.write(json.dumps({"from": a, "to": b}, ensure_ascii=False) + "\n")
                fh.flush()
                stats["sidecars" if i else "moved"] += 1
                if i == 0 and m.get("series_key"):
                    landed.setdefault(Path(b).parts[0], Counter())[m["series_key"]] += 1
    # leave a marker in each tidied show folder so later runs know what it is
    meta = plan.get("series_meta", {})
    for folder, keys in landed.items():
        key = keys.most_common(1)[0][0]
        if key in meta and not Path(folder).is_absolute():
            write_marker(root / folder, meta[key])
    return undo, stats


def undo_log(path: Path, log=print, root: Path | None = None) -> dict:
    root = Path(root or path.parent.parent)          # <root>/.tapehead/undo-*.jsonl
    lines = [json.loads(l) for l in path.read_text().splitlines() if l.strip()]
    stats = {"restored": 0, "problems": 0}
    for entry in reversed(lines):
        src, dst = root / entry["from"], root / entry["to"]
        if dst.exists() and not src.exists():
            _move(dst, src)
            stats["restored"] += 1
        else:
            stats["problems"] += 1
            log(f"couldn't restore {entry['to']} -> {entry['from']}")
    path.rename(path.with_suffix(".undone.jsonl"))
    return stats


def find_clutter(root: Path, keep_subs: bool = False, protect: list[str] | None = None) -> tuple[list[Path], list[Path]]:
    """Return (files, folders) to delete.

    files:   torrent junk (.txt/.nfo/.db/.url…), sample clips, and subtitles unless keep_subs
    folders: folders that hold no video at all once those files are gone (artwork-only leftovers too)
    Never touches .tapehead or anything under a protected path.
    """
    protect = [p.rstrip("/") for p in (protect or [])]

    def protected(p: Path) -> bool:
        rel = str(p.relative_to(root))
        return rel.startswith(".tapehead") or any(rel == x or rel.startswith(x + "/") for x in protect)

    files: list[Path] = []
    for p in root.rglob("*"):
        if not p.is_file() or protected(p):
            continue
        ext = p.suffix.lower()
        name = p.name.lower()
        if name == ".ds_store" or name.startswith("._"):
            files.append(p)
        elif ext in JUNK_EXTS or ext == ".nfo" or (ext in SIDECAR_EXTS and not keep_subs):
            files.append(p)
        elif ext in VIDEO_EXTS and (".sample." in name or name.endswith("-sample" + ext) or name.startswith("sample")):
            files.append(p)

    doomed = set(files)
    folders: list[Path] = []
    for d in sorted((p for p in root.rglob("*") if p.is_dir()), key=lambda p: -len(p.parts)):
        if protected(d):
            continue
        has_video = any(
            f.is_file() and f.suffix.lower() in KEEP_EXTS and f not in doomed for f in d.rglob("*")
        )
        if not has_video:
            folders.append(d)
    tops = [d for d in folders if not any(o in d.parents for o in folders)]
    files = [f for f in files if not any(t in f.parents for t in tops)]
    return sorted(files), sorted(tops)


def delete_clutter(files: list[Path], folders: list[Path]) -> dict:
    stats = {"files": 0, "folders": 0, "errors": 0}
    for f in files:
        try:
            f.unlink()
            stats["files"] += 1
        except OSError:
            stats["errors"] += 1
    for d in folders:
        try:
            shutil.rmtree(d)
            stats["folders"] += 1
        except OSError:
            stats["errors"] += 1
    return stats


def empty_dirs(root: Path) -> list[Path]:
    return find_clutter(root, keep_subs=True)[1]
