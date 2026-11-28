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
"""Verify that the real-host test inventory and its repair pointers survive source changes."""

from __future__ import annotations

import json
import os
import sys
import subprocess
import tomllib
from pathlib import Path
from typing import Any, Iterator

from tools.compatibility.inventory import ROOT, SUPPORT


def _tests(suites: list[dict[str, Any]]) -> Iterator[str]:
    """Read the actual collected titles including parameterized state combinations."""
    for suite in suites:
        yield from (spec["title"] for spec in suite.get("specs", []))
        yield from _tests(suite.get("suites", []))


def test_live_outcomes_have_repair_pointers(tmp_path: Path) -> None:
    """Fail when a real-host scenario or a recorded contract loses its counterpart."""
    output = subprocess.check_output(
        [
            "node",
            "node_modules/@playwright/test/cli.js",
            "test",
            "--config",
            "scripts/compatibility/playwright.config.ts",
            "--list",
            "--reporter=json",
        ],
        cwd=ROOT,
        text=True,
        encoding="utf-8",
        env={
            **os.environ,
            "SUGAR_COMFY_URL": "http://127.0.0.1:1",
            "SUGAR_COMPAT_OUTPUT": str(tmp_path),
        },
    )
    report = json.loads(output)
    assert not report["errors"]
    titles = set(_tests(report["suites"]))
    assert titles
    contracts = tomllib.loads(
        (SUPPORT / "host-contracts.toml").read_text(encoding="utf-8")
    )["contract"]
    for title in titles:
        matches = [c for c in contracts if c["test"] in title]
        assert len(matches) == 1, (title, matches)
    for contract in contracts:
        assert any(contract["test"] in title for title in titles), contract["id"]
        assert all((ROOT / owner).is_file() for owner in contract["owners"])
        assert contract["assumption"] and contract["repair"]


def test_host_source_inventory_matches_checkout() -> None:
    """Require updates to the source ledger when an adapter's observable dependency changes."""
    subprocess.run(
        [sys.executable, "-m", "tools.compatibility.inventory"],
        cwd=ROOT,
        check=True,
    )
