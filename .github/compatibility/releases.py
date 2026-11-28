#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
#
#    This program is distributed in the hope that it will be useful,
#    but WITHOUT ANY WARRANTY; without even the implied warranty of
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#    GNU Affero General Public License for more details.
#
#    You should have received a copy of the GNU Affero General Public License
#    along with this program.  If not, see <https://www.gnu.org/licenses/>.
"""Resolve exact upstream versions before running or caching any compatibility checks."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import re
import subprocess
from pathlib import Path
from typing import Any
from urllib.request import Request, urlopen

CORE = "https://api.github.com/repos/Comfy-Org/ComfyUI"
CONTROL = (
    Path(__file__).resolve().parents[2]
    / "Sugar/tests/comfy_contract/support/control.json"
)


def fetch(url: str) -> Any:
    """Read upstream release metadata with bounded requests."""
    headers = {"User-Agent": "SugarCubes-compatibility"}
    if url.startswith(CORE + "/") and (token := os.environ.get("GH_TOKEN")):
        headers["Authorization"] = f"Bearer {token}"
    with urlopen(Request(url, headers=headers), timeout=30) as response:
        return json.load(response)


def core_lane(ref: str, name: str) -> dict[str, str]:
    """Resolve a tag or upstream branch to its immutable commit and shipped frontend."""
    commit = fetch(f"{CORE}/commits/{ref}")["sha"]
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("ComfyUI returned an invalid commit identity")
    file = fetch(f"{CORE}/contents/requirements.txt?ref={commit}")
    requirements = base64.b64decode(file["content"]).decode("utf-8")
    pin = re.search(r"(?m)^comfyui-frontend-package==(\d+\.\d+\.\d+)\s*$", requirements)
    if not pin:
        raise ValueError(f"Missing frontend pin on {ref}")
    return {"name": name, "core": commit, "frontend": pin[1]}


def resolve() -> list[dict[str, str]]:
    """Resolve the development control, three recent stable pairs and the latest standalone UI."""
    lanes = [{"name": "control", **json.loads(CONTROL.read_text(encoding="utf-8"))}]
    releases = fetch(CORE + "/releases?per_page=100")
    stable = sorted(
        (r for r in releases if not r["draft"] and not r["prerelease"]),
        key=lambda release: release["published_at"],
        reverse=True,
    )
    for release in stable[:3]:
        tag = release["tag_name"]
        if not re.fullmatch(r"v\d+\.\d+\.\d+", tag):
            raise ValueError(f"Unrecognized ComfyUI release tag {tag!r}")
        lanes.append(core_lane(tag, tag))
    if len(lanes) < 4:
        raise ValueError("Could not resolve three recent ComfyUI releases")
    newest = fetch("https://pypi.org/pypi/comfyui-frontend-package/json")["info"][
        "version"
    ]
    if not re.fullmatch(r"\d+\.\d+\.\d+", newest):
        raise ValueError("PyPI returned an unexpected stable frontend version")
    if newest != lanes[1]["frontend"]:
        lanes.append({**lanes[1], "name": "standalone-frontend", "frontend": newest})
    head = core_lane("HEAD", "upstream-head")
    if head["core"] != lanes[1]["core"]:
        lanes.append(head)
    return lanes


def main() -> None:
    """Publish one immutable matrix and its cache identity to GitHub Actions."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--github-output")
    options = parser.parse_args()
    matrix = json.dumps({"include": resolve()}, separators=(",", ":"))
    if options.github_output:
        trees = subprocess.check_output(
            [
                "git",
                "rev-parse",
                "HEAD:Sugar",
                "HEAD:.github/compatibility",
                "HEAD:.github/workflows/sugar-upstream-compatibility.yml",
            ],
            cwd=CONTROL.parents[4],
        )
        identity = hashlib.sha256(trees + matrix.encode()).hexdigest()
        with open(options.github_output, "a", encoding="utf-8") as output:
            output.write(f"matrix={matrix}\n")
            output.write(f"key={identity}\n")
    else:
        print(matrix)


if __name__ == "__main__":
    main()
