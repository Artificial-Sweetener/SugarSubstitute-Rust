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
"""Apply shared output policy to ordinary Comfy prompt submissions before native validation."""

from __future__ import annotations

from collections.abc import Mapping
from copy import deepcopy
import logging
import math
import uuid

from ...execution.models import ApiPrompt, JsonValue
from .queue_policy import OutputSavingQueuePolicy

_logger = logging.getLogger(__name__)


class NativeSavingPromptHook:
    """Translate Comfy's dynamic request into validated shared saving preparation."""

    def __init__(self, policy: OutputSavingQueuePolicy) -> None:
        """Bind the same queue policy used by direct cube execution."""
        self._policy = policy
        self._active = True

    def close(self) -> None:
        """Retire callbacks retained by the native handler list after host shutdown."""
        self._active = False

    def __call__(self, value: object) -> object:
        """Preserve native payload fields and reject preparation failures without allowing unmanaged writes."""
        if (
            not self._active
            or not isinstance(value, Mapping)
            or not isinstance(value.get("prompt"), Mapping)
        ):
            return value
        payload = {
            key: deepcopy(item) for key, item in value.items() if isinstance(key, str)
        }
        prompt_id = payload.get("prompt_id")
        if prompt_id is None:
            prompt_id = str(uuid.uuid4())
            payload["prompt_id"] = prompt_id
        if not isinstance(prompt_id, str):
            return payload
        try:
            prompt = _prompt(payload["prompt"])
            extra = _json_record(payload.get("extra_data", {}))
            pnginfo = extra.get("extra_pnginfo")
            workflow = pnginfo.get("workflow") if isinstance(pnginfo, dict) else None
            workflow_record = (
                {key: item for key, item in workflow.items() if isinstance(key, str)}
                if isinstance(workflow, dict)
                else {}
            )
            payload["prompt"] = self._policy.prepare(
                prompt_id, prompt, workflow=workflow_record, owners={}, extra_data=extra
            )
        except Exception as error:
            _logger.exception(
                "Native output saving preparation rejected the request before any node execution.",
                extra={"prompt_id": prompt_id},
            )
            self._policy.discard(prompt_id)
            payload["prompt"] = {
                "__sugarcubes_output_saving_error__": {
                    "class_type": "SugarCubes.OutputSavingPolicyError",
                    "inputs": {"message": str(error)},
                    "_meta": {"title": "Output saving"},
                }
            }
        return payload


def _prompt(value: object) -> ApiPrompt:
    """Narrow dynamic prompt nodes before they enter typed policy preparation."""
    if not isinstance(value, Mapping):
        raise ValueError("Comfy saving prompt must be an object.")
    result: ApiPrompt = {}
    for key, node in value.items():
        if (
            not isinstance(key, str)
            or not isinstance(node, Mapping)
            or any(not isinstance(field, str) for field in node)
        ):
            raise ValueError(
                "Comfy saving prompt nodes must have string keys and object values."
            )
        result[key] = {
            field: item for field, item in node.items() if isinstance(field, str)
        }
    return result


def _json_record(value: object) -> dict[str, JsonValue]:
    """Keep only validated JSON metadata instead of propagating arbitrary host objects."""
    narrowed = _json_value(value)
    if not isinstance(narrowed, dict):
        raise ValueError("Comfy queue metadata must be an object.")
    return narrowed


def _json_value(value: object) -> JsonValue:
    """Validate the request's finite JSON shape at the external host boundary."""
    if isinstance(value, float) and not math.isfinite(value):
        raise ValueError("Comfy queue metadata contains a nonfinite number.")
    if value is None or isinstance(value, str | bool | int | float):
        return value
    if isinstance(value, list):
        return [_json_value(item) for item in value]
    if isinstance(value, Mapping) and all(isinstance(key, str) for key in value):
        return {
            key: _json_value(item)
            for key, item in value.items()
            if isinstance(key, str)
        }
    raise ValueError("Comfy queue metadata contains a non-JSON value.")
