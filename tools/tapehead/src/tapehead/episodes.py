"""Match one parsed file to a TMDB episode, using the title first and numbers second."""

from __future__ import annotations

import re
from dataclasses import dataclass

from rapidfuzz import fuzz

from .parse import Parsed

SPLIT_RE = re.compile(r"\s*(?: / | & |; |, | - | \+ |\(\d\)\s*|: )\s*")
PAREN_RE = re.compile(r"\s*\(([^()]{4,})\)\s*")


def _n(text: str) -> str:
    text = text.lower().replace("&", "and")
    text = re.sub(r"\bpart\s*(\d)", r"\1", text)
    text = re.sub(r"\((\d)\)", r"\1", text)
    return re.sub(r"[^a-z0-9]+", " ", text).strip()


def _parts(text: str) -> list[str]:
    """The whole title plus its pieces: 'Winter Olympics (Gadget in Winterland)' also tries each name."""
    pieces = [p for chunk in PAREN_RE.split(text) for p in SPLIT_RE.split(chunk)]
    parts = [p for p in pieces if p and len(_n(p)) >= 3]
    return [text] + parts if len(parts) > 1 else [text]


def title_score(hint: str, ep_name: str, show_name: str = "") -> float:
    if not hint or not ep_name:
        return 0.0
    h = hint
    if show_name and _n(h).startswith(_n(show_name)):  # "Pinky And The Brain - Win Big"
        h = h[len(show_name):].strip(" -")
    best = 0.0
    for hp in _parts(h):
        for ep in _parts(ep_name):
            a, b = _n(hp), _n(ep)
            if not a or not b:
                continue
            s = fuzz.ratio(a, b)
            # substring-style matching only when the two are of similar length, so that
            # "Don Smurfo" can't pass for "Return to Don Smurfo"
            if min(len(a), len(b)) >= 8 and min(len(a), len(b)) / max(len(a), len(b)) >= 0.8:
                s = max(s, fuzz.partial_ratio(a, b) - 4)
            best = max(best, s)
    return best


@dataclass
class EpisodeMatch:
    season: int | None
    episode: int | None
    episode_end: int | None
    segment: str | None
    title: str
    confidence: str      # high | medium | low | none
    reason: str
    source: str = "none"  # title | number | both | range | local
    score: float = 0.0


QUALIFIER_RE = re.compile(
    r"\(([^()]*\b(?:version|cut|uncut|uncensored|extended|banned|unaired|director'?s|edit|original|alternate|remaster(?:ed)?)\b[^()]*)\)",
    re.IGNORECASE,
)


def _keep_qualifiers(hint: str, title: str) -> str:
    """Don't lose '(Nick Version)' / '(The Banned Episode)' notes that were in the original filename."""
    for q in QUALIFIER_RE.findall(hint or ""):
        if _n(q) not in _n(title):
            title = f"{title} ({q.strip()})"
    return title


PART_SPLIT_RE = re.compile(r"\s+(?:&|and|\+|/)\s+|\s*,\s+|\s+-\s+", re.IGNORECASE)


def _multi_part(f: Parsed, eps: list[dict], show_name: str) -> EpisodeMatch | None:
    """'The Little Pink Book & Field Trip' -> S01E03-E04 when TMDB lists each story separately."""
    hint = f.title_hint
    if show_name and _n(hint).startswith(_n(show_name)):
        hint = hint[len(show_name):].strip(" -")
    parts = [p for p in PART_SPLIT_RE.split(hint) if len(_n(p)) >= 3]
    if len(parts) < 2 or len(parts) > 4:
        return None
    found = []
    for p in parts:
        scored = sorted(((fuzz.ratio(_n(p), _n(e["name"])), e) for e in eps), key=lambda x: -x[0])
        if not scored or scored[0][0] < 85:
            return None
        found.append(scored[0][1])
    if len({id(e) for e in found}) != len(found) or len({e["season"] for e in found}) != 1:
        return None
    nums = sorted(e["episode"] for e in found)
    if nums[-1] - nums[0] != len(nums) - 1:
        return None
    ordered = sorted(found, key=lambda e: e["episode"])
    title = " & ".join(e["name"] for e in ordered)
    return EpisodeMatch(ordered[0]["season"], nums[0], nums[-1], None, title, "high",
                        f"each story matched TMDB separately (E{nums[0]:02}-E{nums[-1]:02})", "range", 95)


def match_episode(f: Parsed, eps: list[dict], show_name: str = "") -> EpisodeMatch:
    by_num = {(e["season"], e["episode"]): e for e in eps}
    regular = [e for e in eps if e["season"] > 0]
    if f.title_hint and not f.segment:
        mp = _multi_part(f, eps, show_name)
        if mp:
            return mp

    # number-based candidate
    num_ep = None
    if f.season is not None and f.episode is not None:
        num_ep = by_num.get((f.season, f.episode))
    elif f.absolute is not None:
        if f.season and (f.season, f.absolute) in by_num and f.absolute < 40:
            num_ep = by_num[(f.season, f.absolute)]
        elif 0 < f.absolute <= len(regular):
            num_ep = regular[f.absolute - 1]

    # title-based candidate
    title_ep, title_sc, second = None, 0.0, 0.0
    if f.title_hint and len(_n(f.title_hint)) >= 3:
        scored = sorted(((title_score(f.title_hint, e["name"], show_name), e) for e in eps), key=lambda x: -x[0])
        if scored:
            title_sc, title_ep = scored[0]
            second = scored[1][0] if len(scored) > 1 else 0.0

    strong_title = title_ep is not None and title_sc >= 90 and title_sc - second >= 5
    ok_title = title_ep is not None and title_sc >= 75

    def build(ep: dict, conf: str, reason: str, source: str = "number", score: float = 0.0) -> EpisodeMatch:
        end = None
        if f.episode_end and ep["season"] == (f.season if f.season is not None else ep["season"]):
            end = ep["episode"] + (f.episode_end - (f.episode or ep["episode"]))
        title = ep["name"]
        if end:
            nxt = by_num.get((ep["season"], end))
            if nxt and nxt["name"] and nxt["name"] != title:
                title = f"{title} & {nxt['name']}"
        segment = f.segment
        if segment:
            stories = [p for p in re.split(r"\s+/\s+|;\s+", ep["name"]) if p.strip()]
            if len(stories) > 1 and f.title_hint:
                # TMDB lists both halves as one episode: keep the letter and this half's own title
                title = max(stories, key=lambda p: title_score(f.title_hint, p))
            else:
                # TMDB lists this story as its own episode, so the a/b letter no longer applies
                segment = None
        title = _keep_qualifiers(f.title_hint, title)
        return EpisodeMatch(ep["season"], ep["episode"], end, segment, title, conf, reason, source, score)

    if num_ep and title_ep and num_ep is title_ep and ok_title:
        return build(num_ep, "high", "number and title agree", "both", title_sc)
    if strong_title:
        if num_ep and num_ep is not title_ep:
            return build(title_ep, "high", f"title match ({title_sc:.0f}); renumbered from file's numbering", "title", title_sc)
        return build(title_ep, "high", f"title match ({title_sc:.0f})", "title", title_sc)
    if num_ep and not f.title_hint:
        return build(num_ep, "high", "number only (no title in filename)")
    if num_ep and ok_title and title_ep is not num_ep:
        return build(num_ep, "medium", f"number match, but title closer to S{title_ep['season']:02}E{title_ep['episode']:02}")
    if num_ep:
        return build(num_ep, "medium", f"number match; title didn't confirm ({title_sc:.0f})")
    if ok_title:
        return build(title_ep, "medium", f"title only, weak ({title_sc:.0f})", "title", title_sc)
    return EpisodeMatch(None, None, None, None, "", "none", "no episode matched")
