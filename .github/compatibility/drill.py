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
"""Prove that the real classifier reports a deliberately enlarged toolbar on a working host."""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from report import summarize

ROOT = Path(__file__).resolve().parents[2]


def main() -> int:
    """Serve the same installed UI twice, changing only the candidate's toolbar CSS."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--comfy", type=Path, required=True)
    parser.add_argument("--python", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    evidence = args.output.resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    interpreter = args.python.resolve()
    static = subprocess.check_output(
        [
            str(interpreter),
            "-c",
            "import importlib.util as u; print(u.find_spec('comfyui_frontend_package').submodule_search_locations[0])",
        ],
        text=True,
        timeout=30,
    ).strip()
    with tempfile.TemporaryDirectory(
        dir=evidence.parent, prefix="toolbar-drill-"
    ) as temp:
        frontend = Path(temp) / "frontend"
        shutil.copytree(Path(static) / "static", frontend)
        index = frontend / "index.html"
        original = index.read_text(encoding="utf-8")
        if "</head>" not in original:
            raise ValueError(
                "Cannot apply the simulated upstream stylesheet to this frontend"
            )
        index.write_text(
            original.replace(
                "</head>",
                "<style>[data-sc-substitute-toolbar-line]{min-height:52px!important}</style></head>",
                1,
            ),
            encoding="utf-8",
        )
        matrix = []
        for name in ["control", "deliberate-52px"]:
            target = evidence / name
            override = ["--frontend-root", str(frontend)] if name != "control" else []
            run = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "tools.compatibility.host",
                    "--comfy",
                    str(args.comfy.resolve()),
                    "--python",
                    str(interpreter),
                    "--output",
                    str(target),
                    *override,
                    "--",
                    "--project",
                    "chromium",
                    "--grep",
                    "44px toolbar",
                ],
                cwd=ROOT / "Sugar",
                check=False,
                timeout=800,
            )
            versions = json.loads(
                (target / "versions.json").read_text(encoding="utf-8")
            )
            matrix.append({"name": name, **versions})
            if (run.returncode == 0) != (name == "control"):
                raise RuntimeError(
                    f"Unexpected {name} exit {run.returncode}; inspect {target}"
                )
    body, failed, complete = summarize({"include": matrix}, evidence, evidence.as_uri())
    (evidence / "alert.md").write_text(body, encoding="utf-8")
    if not (
        complete
        and failed
        and "New divergence" in body
        and "52px" in body
        and "44px" in body
    ):
        raise RuntimeError(
            "The alert did not identify the specific injected geometry change"
        )
    print(f"Confirmed 44px control and 52px candidate: {evidence / 'alert.md'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
