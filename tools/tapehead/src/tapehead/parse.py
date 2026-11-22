"""Filename parsing: turn messy episode filenames into structured guesses.

Nothing here touches the network or the disk; it works purely on paths so it
can be tested against a plain list of filenames.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from pathlib import PurePosixPath

VIDEO_EXTS = {".mkv", ".mp4", ".avi", ".m4v", ".mov", ".mpg", ".mpeg", ".wmv", ".ts", ".m2ts", ".webm", ".flv", ".divx", ".ogm"}
SIDECAR_EXTS = {".srt", ".sub", ".idx", ".ass", ".ssa", ".vtt", ".nfo"}
JUNK_EXTS = {".txt", ".db", ".url", ".exe", ".lnk", ".ds_store"}
DVD_EXTS = {".vob", ".ifo", ".bup"}

# Words that are release noise rather than part of a title.
NOISE = r"""
    \d{3,4}p | \d{3,4}x\d{3,4} | 4k | uhd | hdr |
    web[ -]?dl | web[ -]?rip | webrip | hdtv | tvrip | dvd[ -]?rip | dvdrip | re[ -]?dvdrip | re[ -]?webrip |
    blu[ -]?ray | bdrip | brrip | hmax | amzn | dsnp | dnsp | nf | dcu |
    x26[45] | h[ .]?26[45] | hevc | avc | xvid | divx | aac(?:2[ .]0)? | ac3 | dd5[ .]1 | dts | mp3 |
    uncensored | complete | proper | repack | remastered | multi[- ]?grp | sample
"""
NOISE_RE = re.compile(rf"\b(?:{NOISE})\b", re.IGNORECASE | re.VERBOSE)
BRACKETS_RE = re.compile(r"\[[^\]]*\]|\{[^}]*\}")
YEAR_RE = re.compile(r"\b(19[3-9]\d|20[0-4]\d)\b")
MONTHS = r"jan|feb|mar|apr|may|jun|jul|aug|sep|sept|oct|nov|dec"

# --- episode token patterns (tried in order, on a cleaned stem) -------------
# S01E02, S1E2, S01 E02, S01.E02, S1-E1, S02E024, S03E16 E17, S01E01-E02, S01E01a
SXE_RE = re.compile(
    r"\bS(?P<s>\d{1,2})\s*[-. ]?\s*E(?P<e>\d{1,3})(?P<seg>[ab])?"
    r"(?:\s*[-&+ ]\s*E?(?P<e2>\d{1,3})(?![\dp]))?(?=\b|[ab]\b|_|[A-Z])",
    re.IGNORECASE,
)
# Hey Arnold style movie marker: S05M01
SXM_RE = re.compile(r"\bS(?P<s>\d{1,2})M(?P<m>\d{1,2})\b", re.IGNORECASE)
# 1x02, 1x03a, 3x07-08
NXN_RE = re.compile(r"\b(?P<s>\d{1,2})x(?P<e>\d{2,3})(?P<seg>[ab])?(?:-(?P<e2>\d{2,3}))?\b", re.IGNORECASE)
# Ren & Stimpy style: 1e01a / 1e00
NEN_RE = re.compile(r"(?:^|\b)(?P<s>\d{1,2})e(?P<e>\d{2})(?P<seg>[ab])?(?=\b|-)", re.IGNORECASE)
# Absolute numbers once a common prefix has been stripped: "Ep. 01", "E73", "001", "- 39"
ABS_RE = re.compile(
    r"^\s*(?:-\s*)?(?:ep(?:isode)?\.?\s*|e)?(?P<n>\d{1,3})(?P<seg>[ab])?(?!\d)(?:\s*-\s*|\s+|$)(?P<rest>.*)$",
    re.IGNORECASE,
)
# "Show - 01 - Title", "Show - Ep. 01 - Title", "Show - 39 Title"
NAME_DASH_NUM_RE = re.compile(
    r"^(?P<name>.*?[A-Za-z].*?)\s+-\s+(?:ep(?:isode)?\.?\s*)?(?P<n>\d{1,3})(?P<seg>[ab])?(?!\d)\s*(?:-\s*|\s+)(?P<rest>.*[A-Za-z].*)$",
    re.IGNORECASE,
)
MOVIE_RE = re.compile(r"\b(the\s+)?movie\b|\bfilm\b", re.IGNORECASE)

SEASON_DIR_RES = [
    re.compile(r"(?:^|[\s._-])(?:season|series)[\s._-]*(?P<s>\d{1,2})\b", re.IGNORECASE),
    re.compile(r"(?:^|[\s._-])S(?P<s>\d{1,2})(?:\b|$)", re.IGNORECASE),
    re.compile(r"^(?P<s>\d{1,2})$"),
]
EXTRAS_DIR_RE = re.compile(r"^(extras?|featurettes?|bonus|behind the scenes)\b", re.IGNORECASE)
SPECIALS_DIR_RE = re.compile(r"^(specials?|extras?|bonus)\b", re.IGNORECASE)


@dataclass
class Parsed:
    path: str                      # path relative to the library root
    kind: str                      # episode | movie | dvd | sidecar | junk | unknown
    name_hint: str = ""            # show name as written in the filename/folder
    year_hint: int | None = None
    season: int | None = None
    episode: int | None = None
    episode_end: int | None = None # for double episodes S01E01-E02
    segment: str | None = None     # 'a' / 'b' half-episodes
    absolute: int | None = None    # absolute number when no season given
    title_hint: str = ""
    aired_year: int | None = None   # a year seen next to the episode title
    pattern: str = ""              # which rule matched (for debugging)
    notes: list[str] = field(default_factory=list)

    @property
    def ext(self) -> str:
        return PurePosixPath(self.path).suffix.lower()


def clean_spaces(text: str) -> str:
    text = text.replace("_", " ").replace("`", "'")
    # dots between words become spaces, but keep initialisms like M.A.S.K. and T.A.S
    text = re.sub(r"(?<=[A-Za-z0-9']{2})\.(?=[A-Za-z0-9])", " ", text)
    text = re.sub(r"(?<=\d)\.(?=[A-Za-z])", " ", text)
    text = re.sub(r"\s+", " ", text)
    return text.strip(" -.")


def strip_noise(text: str) -> str:
    """Remove bracketed tags, resolutions, codecs and rip names."""
    text = BRACKETS_RE.sub(" ", text)
    # parenthesised chunks: drop if they look like noise or dates, keep otherwise
    def paren(m: re.Match) -> str:
        inner = m.group(1)
        if NOISE_RE.search(inner) or YEAR_RE.search(inner) or re.search(MONTHS, inner, re.I):
            return " "
        return f" ({inner}) "
    text = re.sub(r"\(([^()]*)\)", paren, text)
    text = NOISE_RE.sub(" ", text)
    text = re.sub(r"-\s*[A-Za-z0-9]+$", lambda m: m.group(0) if " " in m.group(0).strip("- ") else m.group(0), text)
    text = re.sub(r"\s+", " ", text)
    return text.strip(" -.,+&")


STRONG_NOISE_RE = re.compile(
    r"\b(?:\d{3,4}p|\d{3,4}x\d{3,4}|web[ -]?dl|web[ -]?rip|hdtv|tvrip|dvd[ -]?rip|re[ -]?dvdrip|re[ -]?webrip|"
    r"blu[ -]?ray|bdrip|brrip|hmax|amzn|dsnp|x26[45]|h[ .]?26[45]|hevc|xvid|divx)\b",
    re.IGNORECASE,
)


def clean_title(text: str) -> str:
    """Episode title from the text after the episode token. Release info ends the title."""
    text = clean_spaces(text)
    text = BRACKETS_RE.sub(" ", text)
    def paren(m: re.Match) -> str:
        inner = m.group(1)
        if STRONG_NOISE_RE.search(inner) or YEAR_RE.search(inner) or re.search(rf"\b(?:{MONTHS})\b", inner, re.I):
            return " "
        return f"({inner})"
    text = re.sub(r"\(([^()]*)\)", paren, text)
    m = STRONG_NOISE_RE.search(text)
    if m:
        text = text[: m.start()]
    text = re.sub(r"\s+", " ", text).strip(" -.,")
    text = re.sub(r"^\((.*)\)$", r"\1", text)  # Count Duckula: "(Vampire Vacation)"
    text = re.sub(r"(?:^|\s)(?:uncensored|proper|repack|internal|extended)$", "", text, flags=re.I)
    text = re.sub(r"^(?:uncensored|proper|repack|internal)(?:\s|$)", "", text, flags=re.I)
    return text.strip(" -.")


SHOW_NOISE_RE = re.compile(
    r"\b(?:the\s+)?complete\b.*$|\b(?:all|\d+)\s+(?:seasons?|episodes?)\b.*$|\bseasons?\s*\d.*$|"
    r"\bS\d{1,2}\s*-\s*S\d{1,2}\b.*$|\bcollection\b|\bcartoon(?:s)?\b(?=\s*$)|\bin \w+ format\b",
    re.IGNORECASE,
)


def clean_show_name(text: str) -> tuple[str, int | None]:
    """'COW and CHICKEN (1997-1999) - Complete Animated TV Series, S01-S04' -> ('Cow and Chicken', 1997)."""
    raw = clean_spaces(text)
    year = None
    m = YEAR_RE.search(raw)
    if m:
        year = int(m.group(1))
    name = BRACKETS_RE.sub(" ", raw)
    name = re.sub(r"\([^)]*\)", " ", name)
    name = SHOW_NOISE_RE.sub(" ", name)
    name = NOISE_RE.sub(" ", name)
    name = YEAR_RE.sub(" ", name)
    name = re.sub(r"\bS\d{1,2}(?:\s*-\s*S\d{1,2})?\b", " ", name, flags=re.I)
    name = re.sub(r"\s+", " ", name).strip(" -.,!") + ("!" if raw.rstrip().endswith("!") or "!" in name else "")
    name = name.replace("!!", "!").strip()
    name = re.sub(r"\s*-\s*$", "", name)
    name = re.sub(r"(?:\s*-)?\s*\b(?:ep(?:isode)?\.?|e)\s*$", "", name, flags=re.I).strip(" -")
    if name.isupper() or name.islower() or sum(w.isupper() and len(w) > 2 for w in name.split()) >= 1:
        name = smart_title(name)
    return name, year


SMALL_WORDS = {"a", "an", "and", "as", "at", "but", "by", "for", "in", "n", "'n", "of", "on", "or", "the", "to", "vs"}


def smart_title(text: str) -> str:
    words = text.split()
    out = []
    for i, w in enumerate(words):
        lw = w.lower()
        if "." in w and len(w) <= 6 and w.replace(".", "").isalpha():  # M.A.S.K, T.A.S
            out.append(w.upper())
        elif i > 0 and lw in SMALL_WORDS:
            out.append(lw)
        else:
            out.append(lw[:1].upper() + lw[1:])
    return " ".join(out)


def season_from_dir(name: str) -> int | None:
    if SPECIALS_DIR_RE.match(name):
        return 0
    for rx in SEASON_DIR_RES:
        m = rx.search(name)
        if m:
            return int(m.group("s"))
    return None


def is_season_dir(name: str) -> bool:
    return season_from_dir(name) is not None and len(clean_show_name(name)[0]) <= 2 or bool(
        re.match(r"^(season|series|specials?|extras?)\b", name, re.I)
    ) or bool(re.match(r"^\d{1,2}$", name))


def classify_ext(ext: str) -> str | None:
    if ext in VIDEO_EXTS:
        return "video"
    if ext in SIDECAR_EXTS:
        return "sidecar"
    if ext in DVD_EXTS:
        return "dvd"
    if ext in JUNK_EXTS or not ext or len(ext) > 6 or " " in ext:
        return "junk"
    return None


def _split_on(stem: str, m: re.Match) -> tuple[str, str]:
    return stem[: m.start()], stem[m.end():]


def parse_path(rel: str, common_prefix: str = "") -> Parsed:
    p = PurePosixPath(rel)
    ext = p.suffix.lower()
    cls = classify_ext(ext)
    dirs = list(p.parts[:-1])
    if cls == "junk" or p.name.startswith("."):
        return Parsed(rel, "junk")
    if cls == "dvd" or any(d.upper() == "VIDEO_TS" for d in dirs):
        return Parsed(rel, "dvd")
    if cls is None:
        return Parsed(rel, "junk", notes=["unrecognised extension"])
    if cls == "sidecar":
        return Parsed(rel, "sidecar")
    stem_raw = p.stem
    if re.search(r"(^|[ ._-])sample$", stem_raw, re.I):
        return Parsed(rel, "junk", notes=["sample clip"])

    stem = clean_spaces(stem_raw)
    folder_season = None
    for d in reversed(dirs):
        s = season_from_dir(d)
        if s is not None:
            folder_season = s
            break

    out = Parsed(rel, "episode")

    m = SXM_RE.search(stem)
    if m:
        before, after = _split_on(stem, m)
        out.kind = "movie"
        out.name_hint, out.year_hint = clean_show_name(before)
        out.title_hint = clean_title(after)
        out.pattern = "SxxMyy"
        _year_from(out, after)
        return out

    for rx, label in ((SXE_RE, "SxxEyy"), (NXN_RE, "NxNN"), (NEN_RE, "NeNN")):
        m = rx.search(stem)
        if not m:
            continue
        before, after = _split_on(stem, m)
        out.season = int(m.group("s"))
        out.episode = int(m.group("e"))
        out.segment = (m.group("seg") or "").lower() or None
        if m.groupdict().get("e2"):
            e2 = int(m.group("e2"))
            if e2 > out.episode and e2 - out.episode <= 3:
                out.episode_end = e2
        out.name_hint, out.year_hint = clean_show_name(before) if before.strip(" -") else ("", None)
        out.title_hint = clean_title(after)
        out.pattern = label
        if folder_season is not None and folder_season != out.season and folder_season != 0:
            out.notes.append(f"folder says season {folder_season}")
        return out

    m = NAME_DASH_NUM_RE.match(stem)
    if m and not common_prefix:
        out.name_hint, out.year_hint = clean_show_name(m.group("name"))
        out.absolute, out.pattern = int(m.group("n")), "name-dash-number"
        out.season = folder_season
        out.segment = (m.group("seg") or "").lower() or None
        out.title_hint = clean_title(m.group("rest"))
        return out

    # Compact "201 title" inside a Season 2 folder -> S02E01 (Rocko)
    work = stem
    if common_prefix and work.lower().startswith(common_prefix.lower()):
        work = work[len(common_prefix):]
        out.name_hint, out.year_hint = clean_show_name(common_prefix)
    m = ABS_RE.match(work)
    if m:
        n = int(m.group("n"))
        rest = m.group("rest")
        if folder_season and n >= 100 and n // 100 == folder_season and n % 100 > 0:
            out.season, out.episode, out.pattern = folder_season, n % 100, "compact"
        else:
            out.absolute, out.pattern = n, "absolute"
            out.season = folder_season
        out.segment = (m.group("seg") or "").lower() or None
        out.title_hint = clean_title(rest)
        _year_from(out, rest)
        if out.title_hint and not re.search(r"[A-Za-z]", out.title_hint):
            out.title_hint = ""
        return out

    if any(EXTRAS_DIR_RE.match(d) for d in dirs):
        out.kind = "extra"
        out.title_hint = clean_title(stem)
        out.pattern = "extra"
        return out

    if MOVIE_RE.search(stem):
        out.kind = "movie"
        out.title_hint = clean_title(stem)
        _year_from(out, stem)
        out.pattern = "movie-keyword"
        return out

    out.kind = "unknown"
    out.title_hint = clean_title(stem)
    return out


def _year_from(out: Parsed, text: str) -> None:
    m = YEAR_RE.search(text)
    if m:
        out.aired_year = int(m.group(1))


def common_token_prefix(stems: list[str]) -> str:
    """Longest shared word-prefix of cleaned stems, e.g. 'ulysses 31' or 'MASK -'."""
    if len(stems) < 3:
        return ""
    toks = [clean_spaces(s).split(" ") for s in stems]
    prefix: list[str] = []
    for col in zip(*toks):
        if all(c.lower() == col[0].lower() for c in col):
            prefix.append(col[0])
        else:
            break
    return (" ".join(prefix) + " ") if prefix else ""
