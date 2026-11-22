"""Tiny TMDB client with an on-disk cache. Only uses the standard library."""

from __future__ import annotations

import hashlib
import json
import os
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

API = "https://api.themoviedb.org/3"
CACHE_DIR = Path(os.environ.get("TAPEHEAD_CACHE", Path.home() / ".cache" / "tapehead" / "tmdb"))
CACHE_TTL = 30 * 24 * 3600
CONFIG_FILE = Path.home() / ".config" / "tapehead" / "config.toml"


class TMDBError(RuntimeError):
    pass


def _read_key() -> str:
    for var in ("TMDB_API_KEY", "TMDB_READ_TOKEN"):
        if os.environ.get(var):
            return os.environ[var].strip()
    if CONFIG_FILE.exists():
        for line in CONFIG_FILE.read_text().splitlines():
            if line.strip().startswith(("tmdb_api_key", "tmdb_read_token")) and "=" in line:
                return line.split("=", 1)[1].strip().strip('"').strip("'")
    raise TMDBError(
        "No TMDB key found. Set TMDB_API_KEY (v3 key or v4 read token) or run `tapehead config --tmdb-key ...`."
    )


def save_key(key: str) -> Path:
    CONFIG_FILE.parent.mkdir(parents=True, exist_ok=True)
    CONFIG_FILE.write_text(f'tmdb_api_key = "{key.strip()}"\n')
    CONFIG_FILE.chmod(0o600)
    return CONFIG_FILE


class TMDB:
    def __init__(self, key: str | None = None, language: str = "en-GB"):
        self.key = key or _read_key()
        self.language = language

    def _get(self, path: str, **params) -> dict:
        params = {k: v for k, v in params.items() if v is not None}
        params.setdefault("language", self.language)
        headers = {"Accept": "application/json"}
        if len(self.key) > 40:  # v4 read access token (JWT)
            headers["Authorization"] = f"Bearer {self.key}"
        else:
            params["api_key"] = self.key
        url = f"{API}{path}?{urllib.parse.urlencode(params)}"

        cache_id = hashlib.sha1(f"{path}?{sorted((k, v) for k, v in params.items() if k != 'api_key')}".encode()).hexdigest()
        cache_file = CACHE_DIR / f"{cache_id}.json"
        if cache_file.exists() and time.time() - cache_file.stat().st_mtime < CACHE_TTL:
            return json.loads(cache_file.read_text())

        req = urllib.request.Request(url, headers=headers)
        for attempt in range(4):
            try:
                with urllib.request.urlopen(req, timeout=20) as resp:
                    data = json.loads(resp.read().decode())
                break
            except urllib.error.HTTPError as e:
                if e.code == 429 and attempt < 3:
                    time.sleep(2 ** attempt)
                    continue
                if e.code == 401:
                    raise TMDBError("TMDB rejected the key (401). Check it at themoviedb.org/settings/api.") from e
                raise TMDBError(f"TMDB error {e.code} for {path}") from e
            except urllib.error.URLError as e:
                if attempt < 3:
                    time.sleep(1 + attempt)
                    continue
                raise TMDBError(f"Couldn't reach TMDB: {e.reason}") from e
        CACHE_DIR.mkdir(parents=True, exist_ok=True)
        cache_file.write_text(json.dumps(data))
        return data

    # --- Movies -------------------------------------------------------------
    def search_movie(self, query: str, year: int | None = None) -> list[dict]:
        res = self._get("/search/movie", query=query, year=year).get("results", [])
        if not res and year:
            res = self._get("/search/movie", query=query).get("results", [])
        return res

    # --- TV -----------------------------------------------------------------
    def search_tv(self, query: str, year: int | None = None) -> list[dict]:
        res = self._get("/search/tv", query=query, first_air_date_year=year).get("results", [])
        if not res and year:
            res = self._get("/search/tv", query=query).get("results", [])
        return res

    def tv(self, tv_id: int) -> dict:
        return self._get(f"/tv/{tv_id}")

    def season(self, tv_id: int, season: int) -> dict:
        return self._get(f"/tv/{tv_id}/season/{season}")

    def all_episodes(self, tv_id: int) -> list[dict]:
        show = self.tv(tv_id)
        eps: list[dict] = []
        for s in show.get("seasons", []):
            data = self.season(tv_id, s["season_number"])
            for e in data.get("episodes", []):
                eps.append(
                    {
                        "season": e["season_number"],
                        "episode": e["episode_number"],
                        "name": e.get("name") or "",
                        "air_date": e.get("air_date"),
                    }
                )
        eps.sort(key=lambda e: (e["season"] == 0, e["season"], e["episode"]))
        return eps
