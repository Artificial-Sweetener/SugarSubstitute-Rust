#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Expose stable fail-closed execution diagnostics."""

from __future__ import annotations


class CubeExecutionError(ValueError):
    """Base one machine-readable execution failure."""

    def __init__(self, code: str, phase: str, message: str) -> None:
        """Retain the stable diagnostic identity beside its explanation."""

        super().__init__(message)
        self.code = code
        self.phase = phase


class CubeWorkflowError(CubeExecutionError):
    """Report untrusted canonical workflow input that cannot be accepted."""

    def __init__(self, code: str, message: str, *, path: str) -> None:
        """Retain the failing workflow path for application diagnostics."""

        super().__init__(code, "workflow", message)
        self.path = path


class CubeTopologyError(CubeExecutionError):
    """Report a Cube-only topology that cannot execute deterministically."""

    def __init__(self, code: str, message: str, instance_ids: tuple[str, ...]) -> None:
        """Identify every Cube instance participating in the topology failure."""

        super().__init__(code, "topology", message)
        self.instance_ids = instance_ids


class CubeLoweringError(CubeExecutionError):
    """Report a canonical workflow that cannot lower without guessing."""

    def __init__(self, code: str, message: str, *, path: str = "$") -> None:
        """Retain the canonical workflow path that failed lowering."""

        super().__init__(code, "lowering", message)
        self.path = path


class CubeInheritanceError(CubeExecutionError):
    """Report a resource binding that cannot inherit deterministically."""

    def __init__(
        self,
        code: str,
        message: str,
        *,
        target_instance_id: str,
        slot: str,
        provider_instance_ids: tuple[str, ...] = (),
    ) -> None:
        """Retain target, resource slot, and competing provider identities."""

        super().__init__(code, "inheritance", message)
        self.target_instance_id = target_instance_id
        self.slot = slot
        self.provider_instance_ids = provider_instance_ids


class CubeInstrumentationError(CubeExecutionError):
    """Report a Cube output that cannot receive a stable execution sink."""

    def __init__(self, code: str, message: str, *, instance_id: str) -> None:
        """Retain the Cube instance whose output identity could not be built."""

        super().__init__(code, "instrumentation", message)
        self.instance_id = instance_id


class CubeQueueError(CubeExecutionError):
    """Report invalid queue policy or an unavailable Comfy execution boundary."""

    def __init__(self, code: str, message: str) -> None:
        """Create one fail-closed queue diagnostic."""

        super().__init__(code, "queue", message)
