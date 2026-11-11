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
"""Verify stable JSON-domain semantic hashing."""

from __future__ import annotations

import pytest

from sugarcubes.workflow.semantic_hash import semantic_hash


def test_hash_is_order_independent_but_type_sensitive() -> None:
    """Preserve JSON value types while normalizing object key order."""

    assert semantic_hash({"b": 2, "a": 1}) == semantic_hash({"a": 1, "b": 2})
    assert semantic_hash({"value": 1}) != semantic_hash({"value": "1"})


def test_hash_rejects_non_json_numeric_values() -> None:
    """Fail closed when untrusted content contains an invalid JSON number."""

    with pytest.raises(ValueError):
        semantic_hash({"value": float("nan")})
