"""GitHub API through the `gh` CLI, cached on disk so a run is resumable and cheap to repeat."""

import gzip
import hashlib
import json
import os
import re
import subprocess
import sys
import time

HTTP_STATUS = re.compile(r"HTTP (\d{3})")


class GH:
    def __init__(self, repo, cache_dir, offline=False):
        self.repo = repo
        self.dir = os.path.join(cache_dir, "api")
        self.offline = offline
        self.calls = 0
        os.makedirs(self.dir, exist_ok=True)

    def _file(self, key, ext):
        h = hashlib.sha1(key.encode()).hexdigest()
        return os.path.join(self.dir, h[:2], h + ext)

    def _run(self, args):
        """Runs `gh api ...`; returns (status, stdout bytes). Waits out rate limits."""
        for attempt in range(8):
            if self.offline:
                raise RuntimeError("offline: not cached: " + " ".join(args))
            self.calls += 1
            p = subprocess.run(["gh", "api", *args], capture_output=True)
            if p.returncode == 0:
                return 200, p.stdout
            err = p.stderr.decode(errors="replace")
            m = HTTP_STATUS.search(err)
            status = int(m.group(1)) if m else 0
            if "rate limit" in err.lower() or status == 429:
                self._wait_for_rate_limit()
                continue
            if status in (404, 410, 403, 422):
                return status, b""
            # Network blips and 5xx: back off and retry.
            time.sleep(min(60, 2 ** attempt))
        raise RuntimeError(f"gh api {' '.join(args)} failed: {err.strip()}")

    def _wait_for_rate_limit(self):
        p = subprocess.run(["gh", "api", "rate_limit"], capture_output=True, text=True)
        reset = time.time() + 60
        if p.returncode == 0:
            core = json.loads(p.stdout)["resources"]["core"]
            if core["remaining"] > 50:
                time.sleep(60)  # secondary rate limit
                return
            reset = core["reset"] + 5
        wait = max(5, reset - time.time())
        print(f"  rate limited; waiting {wait:.0f}s", file=sys.stderr)
        time.sleep(wait)

    def get(self, path, ttl=None):
        """JSON for `path` (relative to the API root), or None for 404/410/403. Cached; `ttl` seconds
        bounds how old a cached copy may be (None: forever; offline, any copy is used)."""
        f = self._file(path, ".json")
        if os.path.exists(f) and (ttl is None or self.offline or time.time() - os.path.getmtime(f) < ttl):
            with open(f) as fh:
                c = json.load(fh)
            return c["body"]
        status, out = self._run(["-H", "Accept: application/vnd.github+json", path])
        body = json.loads(out) if status == 200 and out.strip() else None
        os.makedirs(os.path.dirname(f), exist_ok=True)
        tmp = f + ".tmp"
        with open(tmp, "w") as fh:
            json.dump({"path": path, "status": status, "body": body}, fh)
        os.replace(tmp, f)
        return body

    def paged(self, path, key, ttl=None, limit=None):
        """Concatenates `key` lists over `per_page=100` pages."""
        out = []
        page = 1
        sep = "&" if "?" in path else "?"
        while True:
            body = self.get(f"{path}{sep}per_page=100&page={page}", ttl=ttl)
            items = (body or {}).get(key, []) if key else (body or [])
            out.extend(items)
            if len(items) < 100 or (limit and len(out) >= limit):
                return out
            page += 1

    def text_gz(self, path):
        """Text body of `path` (job logs), stored gzipped. None when gone (logs expire: 410)."""
        f = self._file(path, ".txt.gz")
        miss = f + ".missing"
        if os.path.exists(f):
            with gzip.open(f, "rt", errors="replace") as fh:
                return fh.read()
        if os.path.exists(miss):
            return None
        status, out = self._run(["--allow-escape-sequences", path])
        os.makedirs(os.path.dirname(f), exist_ok=True)
        if status != 200:
            with open(miss, "w") as fh:
                fh.write(str(status))
            return None
        with gzip.open(f + ".tmp", "wb") as fh:
            fh.write(out)
        os.replace(f + ".tmp", f)
        return out.decode(errors="replace")

    def download(self, path, dest):
        """Binary body of `path` (artifact zips) to `dest`; False when gone."""
        if os.path.exists(dest):
            return True
        status, out = self._run(["--allow-escape-sequences", path])
        if status != 200:
            return False
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        with open(dest + ".tmp", "wb") as fh:
            fh.write(out)
        os.replace(dest + ".tmp", dest)
        return True

    def cached_json(self, name, ttl, produce):
        """Caches an arbitrary JSON value under `name` (used for `gh pr list`)."""
        f = self._file("named:" + name, ".json")
        if os.path.exists(f) and (ttl is None or self.offline or time.time() - os.path.getmtime(f) < ttl):
            with open(f) as fh:
                return json.load(fh)
        if self.offline:
            raise RuntimeError("offline: not cached: " + name)
        v = produce()
        os.makedirs(os.path.dirname(f), exist_ok=True)
        with open(f + ".tmp", "w") as fh:
            json.dump(v, fh)
        os.replace(f + ".tmp", f)
        return v
