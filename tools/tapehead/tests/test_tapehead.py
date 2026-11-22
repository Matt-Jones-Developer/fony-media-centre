import json
from pathlib import PurePosixPath
from pathlib import Path

import pytest

from tapehead.apply import apply_plan, undo_log
from tapehead.episodes import match_episode
from tapehead.parse import clean_show_name, parse_path
from tapehead.plan import build_plan
from tapehead.state import SeriesEntry
from tapehead.survey import survey


# --- parsing: real filenames from the Cartoons drive -------------------------
@pytest.mark.parametrize(
    "path, season, episode, absolute, segment, end, title",
    [
        ("Count Duckula/Count Duckula S01E02 (Vampire Vacation).mp4", 1, 2, None, None, None, "Vampire Vacation"),
        ("Chip/Chip N Dale S1E10 Three Men and a Booby.mp4", 1, 10, None, None, None, "Three Men and a Booby"),
        ("C/Season 1 (1997)/COW and CHICKEN - S01 E02 - The Girl's Bathroom (480p - DVDRip).mp4", 1, 2, None, None, None, "The Girl's Bathroom"),
        ("D/S02/Dexter`s Laboratory S02E024.mkv", 2, 24, None, None, None, ""),
        ("T/Season 1/Transformers S1-E13The ultimate doom - Part three revival.mp4", 1, 13, None, None, None, "The ultimate doom - Part three revival"),
        ("T/Season 4/E97 The rebirth part 2.avi", 4, None, 97, None, None, "The rebirth part 2"),
        ("P/Pinky And The Brain - S3E16 E17 - Mice Don't Dance, Brain Drained.avi", 3, 16, None, None, 17, "Mice Don't Dance, Brain Drained"),
        ("R/1/1e01a-Stimpy's Big Day.mkv", 1, 1, None, "a", None, "Stimpy's Big Day"),
        ("Rocko/Rockos Modern Life Season 1/Rocko's Modern Life - 1x03a - Jet Scream (doc).mpg", 1, 3, None, "a", None, "Jet Scream (doc)"),
        ("Th/Thundercats.Series1/Thundercats.S01.E43.mp4", 1, 43, None, None, None, ""),
        ("R/The Raccoons S01/The.Raccoons.S01E11.720p.WEBRip.x264-GalaxyTV.mkv", 1, 11, None, None, None, ""),
        ("SP/South.Park.S02E16.Merry.Christmas.Charlie.Manson.1080p.WEB-DL.AAC2.0.H.264-CtrlHD.mkv", 2, 16, None, None, None, "Merry Christmas Charlie Manson"),
        ("F/Futurama - Season 3/Futurama - S03E09 - The Birdbot Of Ice-Catraz [dd].avi", 3, 9, None, None, None, "The Birdbot Of Ice-Catraz"),
        ("MASK/MASK - 39 Green Nightmare[SG1].avi", None, None, 39, None, None, "Green Nightmare"),
        ("S/Spider-Man - 1981 Animated series - Paralias/Spider-Man_ 1981 - 02 - Dr. Doom_ Master of the World.avi", None, None, 2, None, None, "Dr. Doom Master of the World"),
        ("PP/056 Pink-A-Rella (Jan 8, 1969).mp4", None, None, 56, None, None, "Pink-A-Rella"),
    ],
)
def test_parse(path, season, episode, absolute, segment, end, title):
    f = parse_path(path)
    assert (f.season, f.episode, f.absolute, f.segment, f.episode_end, f.title_hint) == (
        season, episode, absolute, segment, end, title
    )


def test_ulysses_common_prefix():
    files = [f"ulysses.31/ulysses.31.{n:02}.{t}-dvdrip.xvid.[merchant].avi" for n, t in [(1, "vengeance.of.the.gods"), (2, "the.lost.planet"), (3, "the.black.sphere")]]
    sv = survey(files)
    assert len(sv.series) == 1
    s = sv.series[0]
    assert s.name == "Ulysses 31"
    assert [f.absolute for f in s.files] == [1, 2, 3]
    assert s.files[1].title_hint == "the lost planet"


def test_rocko_compact_numbers():
    d = "Rocko/Rockos Modern Life Season 2/"
    sv = survey([d + "rocko's modern life 201 i have no son! [dummy].avi", d + "rocko's modern life 205 boob tubed - commuted sentence [dummy].avi", d + "rocko's modern life 213 short story - eyes-capades [dummy].avi"])
    f = sv.series[0].files
    assert [(x.season, x.episode) for x in f] == [(2, 1), (2, 5), (2, 13)]
    assert f[0].title_hint == "i have no son!"


def test_show_names():
    assert clean_show_name("COW and CHICKEN (1997-1999) - Complete Animated TV Series, S01-S04 - 480p DVDRip x264") == ("Cow and Chicken", 1997)
    assert clean_show_name("The Smurfs Complete Seasons 1-9 dvdrip")[0] == "The Smurfs"
    assert clean_show_name("Inspector Gadget 1983 Complete Seasons 1 and 2 ") == ("Inspector Gadget", 1983)


def test_hulk_series_split_by_folder_year():
    files = [
        "Hulk (1982-2016)/1a. 1982-83 Series (720p)/The Incredible Hulk - Ep. 01 - Tomb of the Unknown Hulk (1982 - 720p WebRip).mp4",
        "Hulk (1982-2016)/1a. 1982-83 Series (720p)/The Incredible Hulk - Ep. 02 - Prisoner of the Monster (1982 - 720p WebRip).mp4",
        "Hulk (1982-2016)/1a. 1982-83 Series (720p)/The Incredible Hulk - Ep. 12 - The Boy Who Saw Tomorrow (1983 - 720p WebRip).mp4",
        "Hulk (1982-2016)/1b. 1996-97 T.A.S (480p)/Season 1 (1996-97)/The Incredible Hulk (T.A.S) - S01 E06 - Innocent Blood (1996 - 480p Web-DL).mp4",
    ]
    sv = survey(files)
    assert sorted((s.year, len(s.files)) for s in sv.series) == [(1982, 3), (1996, 1)]


# --- episode matching --------------------------------------------------------
EPS = [
    {"season": 0, "episode": 1, "name": "Big House Blues"},
    {"season": 1, "episode": 1, "name": "Stimpy's Big Day / The Big Shot!"},
    {"season": 1, "episode": 2, "name": "Robin Hoek / Nurse Stimpy"},
    {"season": 1, "episode": 3, "name": "Space Madness / The Boy Who Cried Rat"},
]


def test_title_beats_wrong_number():
    f = parse_path("R/1/1e03-Nurse Stimpy.mkv")
    m = match_episode(f, EPS)
    assert (m.season, m.episode, m.confidence) == (1, 2, "high")
    assert "renumbered" in m.reason


def test_segment_keeps_own_title():
    f = parse_path("R/1/1e01b-The Big Shot.mkv")
    m = match_episode(f, EPS)
    assert (m.season, m.episode, m.segment, m.title) == (1, 1, "b", "The Big Shot!")


def test_pilot_found_in_specials():
    f = parse_path("R/1/1e00-Big House Blues (Nick Version).mkv")
    m = match_episode(f, EPS)
    assert (m.season, m.episode) == (0, 1)


def test_absolute_maps_across_seasons():
    eps = [{"season": s, "episode": e, "name": f"Ep {s}-{e}"} for s in (1, 2) for e in range(1, 4)]
    f = parse_path("M/MASK - 05 - Something [SG1].avi")
    m = match_episode(f, eps)
    assert (m.season, m.episode) == (2, 2)
    assert m.confidence == "medium"  # number only, title didn't confirm


# --- plan / apply / undo round trip -------------------------------------------
def test_plan_apply_undo(tmp_path: Path):
    show = tmp_path / "Ren and Stimpy DVDRip" / "1"
    show.mkdir(parents=True)
    for n in ("1e01a-Stimpy's Big Day.mkv", "1e01a-Stimpy's Big Day.srt", "1e02b-Nurse Stimpy.mkv", "Torrent.txt"):
        (show / n).write_text(n)
    files = [str(p.relative_to(tmp_path)) for p in tmp_path.rglob("*") if p.is_file()]
    sv = survey(files)
    entries = [SeriesEntry(key=s.key, name=s.name, tmdb_id=1, tmdb_name="The Ren & Stimpy Show", year=1991) for s in sv.series]
    plan = build_plan(tmp_path, sv, entries, lambda _id: EPS)
    targets = sorted(m["to"] for m in plan["moves"])
    assert targets == [
        "The Ren & Stimpy Show (1991)/Season 01/The Ren & Stimpy Show (1991) - S01E01a - Stimpy's Big Day.mkv",
        "The Ren & Stimpy Show (1991)/Season 01/The Ren & Stimpy Show (1991) - S01E02b - Nurse Stimpy.mkv",
    ]
    undo, stats = apply_plan(json.loads(json.dumps(plan)), log=lambda *_: None)
    assert stats["moved"] == 2 and stats["sidecars"] == 1
    assert (tmp_path / "The Ren & Stimpy Show (1991)/Season 01/The Ren & Stimpy Show (1991) - S01E01a - Stimpy's Big Day.srt").exists()
    undo_log(undo, log=lambda *_: None)
    assert (show / "1e01a-Stimpy's Big Day.srt").exists()
    assert not (tmp_path / "The Ren & Stimpy Show (1991)/Season 01/The Ren & Stimpy Show (1991) - S01E02b - Nurse Stimpy.mkv").exists()


def _touch(root: Path, rels: list[str]) -> list[str]:
    for r in rels:
        p = root / r
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text("x")
    return rels


def test_movies_extras_and_forced_specials(tmp_path: Path):
    files = _touch(tmp_path, [
        "Hey Arnold! S01-S05 (1996-)/Hey Arnold! S01 (360p)/Hey Arnold! S01E06 Heat & Snow.mp4",
        "Hey Arnold! S01-S05 (1996-)/Hey Arnold! S05M01 The Movie (2002 360p re-webrip).mp4",
        "Pinky and the Brain/Pinky And The Brain - S1E1 - Das Mouse.avi",
        "Pinky and the Brain/Animaniacs - S1E15 - Pinky And The Brain - Battle for the Planet.avi",
        "Ren DVDRip/1/1e01a-Stimpy's Big Day.mkv",
        "Ren DVDRip/1/Extra/Sven Hoek Pencil Test.mkv",
    ])
    sv = survey(files)
    entries = []
    for s in sv.series:
        e = SeriesEntry(key=s.key, name=s.name, year=s.year, mode="local", files=len(s.files))
        if s.name == "Animaniacs":
            e.name, e.year, e.force_season = "Pinky and the Brain", 1995, 0
        elif s.name.startswith("Pinky"):
            e.name, e.year = "Pinky and the Brain", 1995
        entries.append(e)
    plan = build_plan(tmp_path, sv, entries, lambda _: [])
    to = {Path(m["from"]).name: m["to"] for m in plan["moves"]}
    assert to["Hey Arnold! S05M01 The Movie (2002 360p re-webrip).mp4"] == "Hey Arnold! (1996)/Movies/Hey Arnold! The Movie (2002).mp4"
    assert to["Sven Hoek Pencil Test.mkv"].endswith("/Extras/Sven Hoek Pencil Test.mkv")
    assert to["Animaniacs - S1E15 - Pinky And The Brain - Battle for the Planet.avi"].startswith("Pinky and the Brain (1995)/Specials/")
    assert to["Pinky And The Brain - S1E1 - Das Mouse.avi"].startswith("Pinky and the Brain (1995)/Season 01/")


def test_clean_finds_junk_and_empty_folders(tmp_path: Path):
    from tapehead.apply import find_clutter
    _touch(tmp_path, [
        "Show/Season 1/ep1.mkv", "Show/Season 1/ep1.srt", "Show/Torrent downloaded.txt",
        "Old Release/folder.jpg", "Old Release/note.txt",
        "DnD/Disc 1/VIDEO_TS/VTS_01_1.VOB",
        ".tapehead/plan.json",
    ])
    files, folders = find_clutter(tmp_path)
    rel = lambda ps: sorted(str(p.relative_to(tmp_path)) for p in ps)
    assert rel(folders) == ["Old Release"]
    assert rel(files) == ["Show/Season 1/ep1.srt", "Show/Torrent downloaded.txt"]


def _plan_for(tmp_path, rels, eps, name="Show", tmdb_name="Show"):
    _touch(tmp_path, rels)
    sv = survey(rels)
    entries = [SeriesEntry(key=s.key, name=s.name, year=1990, tmdb_id=1, tmdb_name=tmdb_name, files=len(s.files)) for s in sv.series]
    return build_plan(tmp_path, sv, entries, lambda _id: eps)


def test_hey_arnold_two_stories_become_a_range(tmp_path):
    eps = [{"season": 1, "episode": n, "name": t} for n, t in enumerate(
        ["Downtown as Fruits", "Eugene's Bike", "The Little Pink Book", "Field Trip", "Arnold's Hat", "Stoop Kid"], 1)]
    plan = _plan_for(tmp_path, [
        "HA/Hey Arnold! S01 (360p)/Hey Arnold! S01E02 The Little Pink Book & Field Trip.mp4",
        "HA/Hey Arnold! S01 (360p)/Hey Arnold! S01E03 Arnold's Hat & Stoop Kid.mp4",
    ], eps, tmdb_name="Hey Arnold!")
    to = sorted(PurePosixPath(m["to"]).name for m in plan["moves"])
    assert to == [
        "Hey Arnold! (1990) - S01E03-E04 - The Little Pink Book & Field Trip.mp4",
        "Hey Arnold! (1990) - S01E05-E06 - Arnold's Hat & Stoop Kid.mp4",
    ]


def test_title_wins_clash_and_offset_numbering_is_distrusted(tmp_path):
    # TMDB puts the 3-part pilot in Specials, so the files' S1 numbering is 3 ahead
    eps = [{"season": 0, "episode": n, "name": f"More Than Meets the Eye ({n})"} for n in (1, 2, 3)] + \
          [{"season": 1, "episode": n, "name": t} for n, t in enumerate(
              ["Transport to Oblivion", "Roll for It", "Divide and Conquer", "S.O.S. Dinobots"], 1)]
    plan = _plan_for(tmp_path, [
        "T/Season 1/Transformers S1-E1 More than meets the eye day one.mp4",
        "T/Season 1/Transformers S1-E4 Transport to oblivion.mp4",
        "T/Season 1/Transformers S1-E5 Roll for it.mp4",
        "T/Season 1/Transformers S1-E6 Divide & conquer.mp4",
        "T/Season 1/Transformers S1-E7 Some mystery title.mp4",
    ], eps, tmdb_name="The Transformers")
    to = {PurePosixPath(m["from"]).name: (PurePosixPath(m["to"]).name, m["confidence"]) for m in plan["moves"]}
    assert to["Transformers S1-E4 Transport to oblivion.mp4"] == ("The Transformers (1990) - S01E01 - Transport to Oblivion.mp4", "high")
    assert to["Transformers S1-E6 Divide & conquer.mp4"][0].endswith("S01E03 - Divide and Conquer.mp4")
    # a file whose title matched nothing must not be trusted on numbering alone in this series
    assert "Transformers S1-E7 Some mystery title.mp4" not in to or to["Transformers S1-E7 Some mystery title.mp4"][1] == "low"


def test_second_run_is_a_no_op_thanks_to_markers(tmp_path):
    from tapehead import state
    from tapehead.cli import scan
    rels = _touch(tmp_path, ["Ren DVDRip/1/1e01a-Stimpy's Big Day.mkv", "Ren DVDRip/1/1e02-Robin Hoek.mkv"])
    sv = survey(scan(tmp_path))
    entries = state.merge(tmp_path, sv)
    for e in entries:
        e.tmdb_id, e.tmdb_name, e.year = 504, "The Ren & Stimpy Show", 1991
    state.save(tmp_path, entries)
    plan = build_plan(tmp_path, sv, entries, lambda _: EPS)
    apply_plan(plan, log=lambda *_: None)
    assert (tmp_path / "The Ren & Stimpy Show (1991)" / ".tapehead-series.yaml").exists()

    sv2 = survey(scan(tmp_path))
    entries2 = state.merge(tmp_path, sv2)
    tidy = [e for e in entries2 if e.key.startswith("The Ren & Stimpy Show (1991)")]
    assert tidy and tidy[0].tmdb_id == 504
    plan2 = build_plan(tmp_path, sv2, entries2, lambda _: EPS)
    assert plan2["moves"] == []


def test_segment_letter_dropped_when_tmdb_lists_story_alone():
    eps = [{"season": 4, "episode": 17, "name": "Aloha Höek"}, {"season": 4, "episode": 18, "name": "Wiener Barons"}]
    m = match_episode(parse_path("R/4/4e09b-Aloha Hoek.mkv"), eps)
    assert (m.season, m.episode, m.segment, m.title) == (4, 17, None, "Aloha Höek")


def test_version_notes_survive():
    eps = [{"season": 1, "episode": 7, "name": "Big House Blues"}]
    m = match_episode(parse_path("R/1/1e00-Big House Blues (Nick Version).mkv"), eps)
    assert m.title == "Big House Blues (Nick Version)"


def test_unverifiable_number_only_is_left_alone(tmp_path):
    eps = [{"season": 2, "episode": n, "name": f"Ep {n}"} for n in range(1, 26)]  # TMDB: 25, files: 3
    plan = _plan_for(tmp_path, [f"TC/Thundercats.Series2/Thundercats.S02.E{n:02}.mp4" for n in (1, 2, 3)], eps)
    assert plan["moves"] and all(m["confidence"] == "low" and "can't verify numbering" in m["reason"] for m in plan["moves"])


def test_full_season_number_only_is_trusted(tmp_path):
    eps = [{"season": 1, "episode": n, "name": f"Ep {n}"} for n in range(1, 4)]
    plan = _plan_for(tmp_path, [f"R/The Raccoons S01/The.Raccoons.S01E{n:02}.720p.WEBRip.x264-GalaxyTV.mkv" for n in (1, 2, 3)], eps)
    assert len(plan["moves"]) == 3 and all(m["confidence"] == "high" for m in plan["moves"])


def test_sequel_title_does_not_match_the_original():
    eps = [{"season": 6, "episode": 8, "name": "Don Smurfo"}, {"season": 6, "episode": 9, "name": "Something Else"}]
    m = match_episode(parse_path("S/Season 7/The Smurfs S07E08 - Return To Don Smurfo.mkv"), eps)
    assert m.confidence != "high" or (m.season, m.episode) != (6, 8)


def test_alt_title_in_brackets_still_matches():
    eps = [{"season": 1, "episode": 65, "name": "Gadget in Winterland"}, {"season": 1, "episode": 1, "name": "Monster Lake"}]
    m = match_episode(parse_path("IG/Season 1/Inspector Gadget S01E01 Winter Olympics (Gadget in Winterland).mkv"), eps)
    assert (m.season, m.episode, m.confidence) == (1, 65, "high")


def test_uncensored_is_not_a_title():
    assert parse_path("SP/South.Park.S19E02.Uncensored.1080p.WEB-DL.AAC2.0.H.264-NTb.mkv").title_hint == ""
