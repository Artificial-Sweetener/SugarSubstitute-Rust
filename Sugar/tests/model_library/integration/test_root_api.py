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
"""Prove root configuration crosses HTTP without changing running model paths."""

from __future__ import annotations

import asyncio
from pathlib import Path
import subprocess
import sys

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer

from sugarcubes.model_library.root_api import register_model_root_routes
from sugarcubes_model_roots import ModelRootConfiguration, ModelRootStore


def test_root_routes_preserve_running_state_and_reject_invalid_paths(
    tmp_path: Path,
) -> None:
    """Use a real isolated HTTP host and preserve the active registry until restart."""

    async def exercise() -> None:
        """Exercise both public reads and intentional configuration writes."""
        active = tmp_path / "models"
        configuration = ModelRootConfiguration(ModelRootStore(tmp_path), lambda: active)
        routes = web.RouteTableDef()
        register_model_root_routes(routes, configuration)
        app = web.Application()
        app.add_routes(routes)
        async with TestClient(TestServer(app)) as client:
            response = await client.get("/sugarcubes/v1/models/root")
            payload: object = await response.json()
            assert isinstance(payload, dict)
            assert payload["usesDefault"] is True
            response = await client.put(
                "/sugarcubes/v1/models/root",
                json={"mode": "custom", "path": str(tmp_path / "shared")},
            )
            assert response.status == 200
            payload = await response.json()
            assert isinstance(payload, dict)
            assert payload["restartRequired"] is True
            assert payload["activeModelRoot"] == str(active)
            response = await client.put(
                "/sugarcubes/v1/models/root",
                json={"mode": "custom", "path": "relative"},
            )
            assert response.status == 400
            response = await client.put(
                "/sugarcubes/v1/models/root", json={"mode": "default", "path": None}
            )
            assert response.status == 200
            assert configuration.get_status().configured_model_root is None

    asyncio.run(exercise())


def test_prestartup_api_bootstraps_without_node_or_model_imports(
    tmp_path: Path,
) -> None:
    """Run the real public loader in an isolated interpreter outside the checkout."""
    loader = Path(__file__).resolve().parents[3] / "load_model_roots.py"
    program = """import importlib.util, sys
spec = importlib.util.spec_from_file_location('root_api_loader', sys.argv[1])
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
api = module.load_model_root_api()
assert api.MODEL_ROOT_API_VERSION == 1
assert api is module.load_model_root_api()
assert 'sugarcubes' not in sys.modules
assert 'torch' not in sys.modules
assert 'numpy' not in sys.modules
"""
    result = subprocess.run(
        [sys.executable, "-I", "-c", program, str(loader)],
        cwd=tmp_path,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
