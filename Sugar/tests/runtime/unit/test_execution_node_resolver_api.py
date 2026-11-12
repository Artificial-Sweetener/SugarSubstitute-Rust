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
"""Verify the retained public API in isolated Comfy-style package bootstraps."""

from __future__ import annotations

from pathlib import Path
import subprocess
import sys


def test_version_one_public_exports_share_one_registration_lifetime() -> None:
    """Resolve and release callbacks through both public surfaces in a clean process."""
    script = """
from collections.abc import Mapping
import sys
from sugarcubes import host_api, runtime
from sugarcubes.execution.models import ApiPrompt

assert host_api.EXECUTION_NODE_RESOLVER_API_VERSION == 1
assert runtime.EXECUTION_NODE_RESOLVER_API_VERSION == 1
assert host_api.register_execution_node_resolver is runtime.register_execution_node_resolver
assert host_api.unregister_execution_node_resolver is runtime.unregister_execution_node_resolver
for name in (
    'EXECUTION_NODE_RESOLVER_API_VERSION',
    'register_execution_node_resolver',
    'unregister_execution_node_resolver',
):
    assert name in host_api.__all__ and name in runtime.__all__
assert sys.modules['sugarcubes.host_api'] is host_api
seen: list[str] = []
def resolve(node: Mapping[str, object]) -> str | None:
    seen.append(str(node['class_type']))
    return 'Runtime'
prompt: ApiPrompt = {'source': {'class_type': 'Original', 'inputs': {}}}
host_api.register_execution_node_resolver(resolve)
runtime.register_execution_node_resolver(resolve)
try:
    assert runtime.execution_node_resolver_registry().resolve(prompt)['source']['class_type'] == 'Runtime'
    assert seen == ['Original']
finally:
    runtime.unregister_execution_node_resolver(resolve)
assert runtime.execution_node_resolver_registry().resolve(prompt) == prompt
assert seen == ['Original']
"""
    _run_bootstrap(script)


def test_extension_composes_the_public_registry_into_its_active_queue() -> None:
    """Run actual path-derived extension composition with inert external host services."""
    script = """
import asyncio
from collections.abc import Mapping
from contextlib import ExitStack
import importlib
import importlib.util
from pathlib import Path
import sys
from types import SimpleNamespace
from unittest.mock import patch

root = Path.cwd()
package = 'sugarcubes_resolver_entrypoint'
spec = importlib.util.spec_from_file_location(
    package, root / '__init__.py', submodule_search_locations=[str(root)]
)
assert spec is not None and spec.loader is not None
entrypoint = importlib.util.module_from_spec(spec)
sys.modules[package] = entrypoint
ports: list[object] = []
queued: list[object] = []
def build_services(_root: Path, *, execution_port: object) -> SimpleNamespace:
    ports.append(execution_port)
    return SimpleNamespace(execution=SimpleNamespace(project_prepared_inputs=installed))
def installed(*_args: object, **_kwargs: object) -> None:
    pass
async def validate(
    _prompt_id: str, prompt: object, _targets: object
) -> tuple[bool, object, object, object]:
    assert isinstance(prompt, dict)
    assert all(node['class_type'] == 'Runtime' for node in prompt.values())
    return True, None, (), {}
def resolve(_node: Mapping[str, object]) -> str | None:
    return 'Runtime'
server = SimpleNamespace(
    number=0,
    prompt_queue=SimpleNamespace(put=queued.append),
    node_replace_manager=SimpleNamespace(apply_replacements=installed),
)
sys.modules['server'] = SimpleNamespace(PromptServer=server)
sys.modules['execution'] = SimpleNamespace(
    validate_prompt=validate, SENSITIVE_EXTRA_DATA_KEYS=()
)
boundaries = {
    'backend.composition': {'build_backend_services': build_services},
    'backend.routes': {'register_routes': installed},
    'frontend_assets': {'install_canvas_module_mime': installed},
    'host_api': {'set_active_backend_services': installed},
    'model_library.integration': {'install_model_library': installed},
    'output_saving.integration': {
        'install_output_saving': lambda _server: SimpleNamespace(policy=None)
    },
    'input_canvas.infrastructure.http_routes': {'install_input_previews': installed},
    'execution.spatial_probe_http': {'install_spatial_probe_routes': installed},
    'input_canvas.infrastructure.prepared_input_http': {
        'install_prepared_input_route': installed
    },
}
with ExitStack() as patches:
    for name, replacements in boundaries.items():
        module = importlib.import_module(f'{package}.sugarcubes.{name}')
        for operation, replacement in replacements.items():
            patches.enter_context(patch.object(module, operation, replacement))
    runtime = importlib.import_module(f'{package}.sugarcubes.runtime')
    host_api = importlib.import_module('sugarcubes.host_api')
    assert host_api.register_execution_node_resolver is runtime.register_execution_node_resolver
    host_api.register_execution_node_resolver(resolve)
    try:
        spec.loader.exec_module(entrypoint)
        assert len(ports) == 1
        assert isinstance(ports[0], entrypoint.DirectComfyExecutionPort)
        execution = importlib.import_module(f'{package}.sugarcubes.execution')
        fixtures = importlib.import_module('tests.execution.support.execution_fixtures')
        prepared = execution.CubeExecutionCoordinator().prepare(
            execution.CubeExecutionRequest(
                workflow=fixtures.cube_workflow({'a': fixtures.provider_document()})
            )
        )
        receipt = asyncio.run(ports[0].queue(prepared))
    finally:
        host_api.unregister_execution_node_resolver(resolve)
assert receipt.accepted and len(queued) == 1
assert receipt.execution_prompt is not None
assert all(node['class_type'] == 'Runtime' for node in receipt.execution_prompt.values())
"""
    _run_bootstrap(script)


def _run_bootstrap(script: str) -> None:
    """Keep Comfy's path-derived canonical publication separate from pytest's package import."""
    result = subprocess.run(
        [sys.executable, "-c", script],
        cwd=Path(__file__).resolve().parents[3],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    assert result.stderr == "", result.stderr
