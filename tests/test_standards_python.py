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
"""Static standards coverage for Python runtime code."""

from __future__ import annotations

from typing import Any

import importlib.util
from pathlib import Path


def _load_audit_module() -> Any:
    """Load the standards audit script as a reusable test helper."""

    root = Path(__file__).resolve().parents[1]
    module_path = root / "scripts" / "standards_audit.py"
    spec = importlib.util.spec_from_file_location("standards_audit", module_path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_python_standards_audit_passes() -> None:
    """Keep Python exception and docstring policy regressions out of the repo."""

    audit_module = _load_audit_module()
    failures = audit_module.run_audit()
    assert failures == []
