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
"""Prove accepted graph and cube queue items record distinct exact models without execution."""

from __future__ import annotations

from pathlib import Path
from collections.abc import Callable

import pytest

from sugarcubes.model_library.infrastructure.comfy_model_usage import (
    ComfyModelUsageObserver,
)
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from sugarcubes.model_library.domain.model_updates import ModelUpdateState
from tests.model_library.support.comfy_queue import AcceptedPromptQueue
from tests.model_library.support.enrichment import ObservedProvider
from tests.model_library.support.model_roots import FixtureFileRoots


def fixture(
    tmp_path: Path,
) -> tuple[ModelLibraryRuntime, AcceptedPromptQueue, ComfyModelUsageObserver]:
    """Use production usage persistence and catalog composition around a tiny isolated model identity."""
    (tmp_path / "installed.safetensors").write_bytes(b"fixture")
    runtime = ModelLibraryRuntime(
        FixtureFileRoots(tmp_path), enrichment_provider=ObservedProvider()
    )
    runtime.catalog.refresh()
    entry = runtime.repository.list_entries()[0]
    runtime.repository.save_fingerprint(entry.evidence, "a" * 64)
    queue = AcceptedPromptQueue()
    return runtime, queue, ComfyModelUsageObserver(queue, runtime.queued_model_usage)


def test_usage_counts_accepted_compiled_values_once_and_ignores_extra_data(
    tmp_path: Path,
) -> None:
    """Both graph and cube dispatches use the native compiled prompt insertion boundary."""
    runtime, queue, observer = fixture(tmp_path)
    prompt = {
        "1": {"inputs": {"lora_name": "installed.safetensors"}},
        "2": {"inputs": {"lora_name": "installed.safetensors"}},
    }
    item: tuple[object, ...] = (1, "accepted", prompt, {}, ["2"], {})
    queue.put(item)
    assert queue.items == [item]
    assert runtime.model_updates.usage.load().records[0].usage_count == 1
    queue.put((2, "extra-only", {}, {"secret": "installed.safetensors"}, [], {}))
    assert runtime.model_updates.usage.load().records[0].usage_count == 1
    observer.close()
    queue.put(item)
    assert runtime.model_updates.usage.load().records[0].usage_count == 1


def test_rejected_insertions_never_record_usage(tmp_path: Path) -> None:
    """Do not count a valid-looking prompt before its host queue insertion succeeds."""
    runtime, queue, observer = fixture(tmp_path)
    queue.reject = True
    with pytest.raises(ValueError, match="rejected"):
        queue.put(
            (
                1,
                "rejected",
                {"1": {"inputs": {"lora_name": "installed.safetensors"}}},
                {},
                [],
                {},
            )
        )
    assert runtime.model_updates.usage.load().records == () and queue.items == []
    observer.close()


def test_optional_observer_failure_cannot_reject_accepted_prompt(
    tmp_path: Path, caplog: pytest.LogCaptureFixture
) -> None:
    """Log unexpected host-shape failures without changing successful queue acceptance."""
    runtime, queue, observer = fixture(tmp_path)
    queue.put((1, "accepted", "unknown host shape"))
    assert len(queue.items) == 1 and runtime.model_updates.usage.load().records == ()
    assert "after accepted queue insertion" in caplog.text
    observer.close()


def test_cleanup_does_not_clobber_another_extension_wrapper(tmp_path: Path) -> None:
    """Deactivate retained callback chains while preserving later host adapters."""
    runtime, queue, observer = fixture(tmp_path)
    previous = queue.put

    def outer(item: object) -> None:
        """Delegate as a later independent host extension would."""
        previous(item)

    setattr(queue, "put", outer)
    observer.close()
    assert queue.put is outer
    queue.put(
        (
            1,
            "accepted",
            {"1": {"inputs": {"lora_name": "installed.safetensors"}}},
            {},
            [],
            {},
        )
    )
    assert len(queue.items) == 1 and runtime.model_updates.usage.load().records == ()


def test_unexpected_storage_fault_keeps_queue_acceptance_and_traceback(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, caplog: pytest.LogCaptureFixture
) -> None:
    """Prove the exact audited exception boundary cannot turn optional observer faults into rejection."""
    runtime, queue, observer = fixture(tmp_path)

    def fail(
        _transition: Callable[[ModelUpdateState], ModelUpdateState],
    ) -> ModelUpdateState:
        """Simulate an unexpected persistence failure after host acceptance."""
        raise RuntimeError("fixture persistence fault")

    monkeypatch.setattr(runtime.model_updates.usage, "change", fail)
    queue.put(
        (
            1,
            "accepted",
            {"1": {"inputs": {"lora_name": "installed.safetensors"}}},
            {},
            [],
            {},
        )
    )
    assert len(queue.items) == 1
    assert "fixture persistence fault" in caplog.text
    assert any(record.exc_info is not None for record in caplog.records)
    observer.close()
