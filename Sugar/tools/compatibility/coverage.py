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
"""Relate directly asserted behavior and code reached during each real-host scenario."""

from __future__ import annotations

import json
import tomllib
from collections import defaultdict
from pathlib import Path

from tools.compatibility.inventory import ROOT, SUPPORT
from tools.testing.policy import load_test_policy


def record_coverage(output: Path) -> None:
    """Expose every indexed file, including sources that received no observed execution."""
    paths: dict[str, list[str]] = json.loads(
        (SUPPORT / "host-dependencies.json").read_text(encoding="utf-8")
    )
    contracts = tomllib.loads(
        (SUPPORT / "host-contracts.toml").read_text(encoding="utf-8")
    )["contract"]
    areas = load_test_policy(ROOT / "TEST_POLICY.toml").areas
    observed: dict[str, list[str]] = defaultdict(list)
    for file in sorted((output / "source-coverage").glob("*.json")):
        sample = json.loads(file.read_text(encoding="utf-8"))
        for path in sample["observed"]:
            observed[path].append(f"{sample['browser']}: {sample['scenario']}")
    facts = {
        path: {
            "area": [area.name for area in areas if area.owns(path)],
            "assumptions": len(entries),
            "assertions": [
                contract["id"] for contract in contracts if path in contract["owners"]
            ],
            "reached": sorted(set(observed.get(path, []))),
        }
        for path, entries in paths.items()
    }
    (output / "coverage-index.json").write_text(
        json.dumps(facts, indent=2), encoding="utf-8"
    )
