#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Measure dependency acquisition phases through one structured diagnostic."""

from __future__ import annotations

import logging
from collections.abc import Iterator
from contextlib import contextmanager
from time import perf_counter

from ...instrumentation.logger import log_diagnostic

_logger = logging.getLogger(__name__)
_TRACE_MARKER = "SugarCubes dependency acquisition diagnostic"


@contextmanager
def dependency_acquisition_phase(
    *,
    node_id: str,
    operation: str,
    source: str,
) -> Iterator[None]:
    """Measure one external or mutation boundary, including failed attempts."""

    started_at = perf_counter()
    outcome = "failure"
    try:
        yield
        outcome = "success"
    finally:
        log_diagnostic(
            _logger,
            _TRACE_MARKER,
            "sugarcubes_dependency_acquisition_phase_timing",
            {
                "node_id": node_id,
                "operation": operation,
                "source": source,
                "outcome": outcome,
                "duration_ms": round((perf_counter() - started_at) * 1000, 3),
            },
        )


__all__ = ["dependency_acquisition_phase"]
