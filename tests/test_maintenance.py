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
"""Offline maintenance CLI diagnostics tests."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

import pytest

from sugarcubes.backend import maintenance
from sugarcubes.backend.responses import BackendError


class _DependencyCommands:
    """Record maintenance dependency commands and return configured payloads."""

    def __init__(self, result: object, *, failure: BackendError | None = None) -> None:
        """Initialize one command recorder."""

        self.result = result
        self.failure = failure
        self.calls: list[tuple[str, object]] = []

    def readiness(self) -> object:
        """Record one readiness command."""

        self.calls.append(("preflight", None))
        if self.failure is not None:
            raise self.failure
        return self.result

    def repair(
        self,
        *,
        approval_policy: str,
        approved_node_ids: tuple[str, ...] | list[str],
        sync_enabled_repos: bool,
    ) -> object:
        """Record one repair command."""

        self.calls.append(
            (
                "repair",
                {
                    "approval_policy": approval_policy,
                    "approved_node_ids": tuple(approved_node_ids),
                    "sync_enabled_repos": sync_enabled_repos,
                },
            )
        )
        if self.failure is not None:
            raise self.failure
        return self.result

    def sync_and_check(self, payload: dict[str, object]) -> object:
        """Record one sync-and-check command."""

        self.calls.append(("sync-and-check", payload))
        if self.failure is not None:
            raise self.failure
        return self.result


@dataclass(frozen=True)
class _BackendServices:
    """Expose the dependency collaborator used by the maintenance entrypoint."""

    dependencies: _DependencyCommands


@dataclass
class _BuildCapture:
    """Record resolved service-construction paths."""

    extension_root: Path | None = None
    workspace_path: Path | None = None
    custom_nodes_root: Path | None = None

    def build(
        self,
        extension_root: Path,
        *,
        workspace_path: Path,
        custom_nodes_root: Path,
        dependencies: _DependencyCommands,
    ) -> _BackendServices:
        """Record paths and return configured backend services."""

        self.extension_root = extension_root
        self.workspace_path = workspace_path
        self.custom_nodes_root = custom_nodes_root
        return _BackendServices(dependencies=dependencies)


def test_maintenance_crash_writes_structured_diagnostic(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    tmp_path: Path,
) -> None:
    """Unexpected maintenance crashes should still produce machine-readable JSON."""

    def fail_build_services(*args: object, **kwargs: object) -> object:
        """Raise the unexpected failure exercised by this test."""

        _ = args, kwargs
        raise RuntimeError("boom")

    monkeypatch.setattr(maintenance, "build_backend_services", fail_build_services)

    exit_code = maintenance.main(
        ["cube-deps", "sync-and-check", "--workspace", str(tmp_path)]
    )

    payload = json.loads(capsys.readouterr().out)
    assert exit_code == 1
    assert payload["error"] == "SugarCubes maintenance crashed"
    assert payload["diagnostics"][0]["code"] == "maintenance_crashed"
    assert payload["diagnostics"][0]["severity"] == "error"


def test_maintenance_preflight_resolves_workspace_and_returns_success(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    tmp_path: Path,
) -> None:
    """Preflight uses resolved workspace paths and returns zero when ready."""

    dependencies = _DependencyCommands({"ready": True, "requirements": []})
    capture = _BuildCapture()

    def build_services(
        extension_root: Path,
        *,
        workspace_path: Path,
        custom_nodes_root: Path,
    ) -> _BackendServices:
        """Return the preflight command recorder."""

        return capture.build(
            extension_root,
            workspace_path=workspace_path,
            custom_nodes_root=custom_nodes_root,
            dependencies=dependencies,
        )

    monkeypatch.setattr(maintenance, "build_backend_services", build_services)

    exit_code = maintenance.main(
        ["cube-deps", "preflight", "--workspace", str(tmp_path / "workspace")]
    )

    payload = json.loads(capsys.readouterr().out)
    expected_workspace = (tmp_path / "workspace").resolve()
    assert exit_code == 0
    assert payload == {"ready": True, "requirements": []}
    assert dependencies.calls == [("preflight", None)]
    assert capture.extension_root == Path(maintenance.__file__).resolve().parents[1]
    assert capture.workspace_path == expected_workspace
    assert capture.custom_nodes_root == expected_workspace / "custom_nodes"


def _assert_repair_contract(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    tmp_path: Path,
    *,
    extra_args: list[str],
    expected_policy: str,
    expected_approved: tuple[str, ...],
) -> None:
    """Assert one repair approval policy at the dependency service boundary."""

    dependencies = _DependencyCommands({"readinessAfter": {"ready": False}})
    monkeypatch.setattr(
        maintenance,
        "build_backend_services",
        lambda *args, **kwargs: _BackendServices(dependencies=dependencies),
    )

    exit_code = maintenance.main(
        [
            "cube-deps",
            "repair",
            "--workspace",
            str(tmp_path),
            "--sync-enabled-repos",
            *extra_args,
        ]
    )

    assert exit_code == 2
    assert json.loads(capsys.readouterr().out) == {"readinessAfter": {"ready": False}}
    assert dependencies.calls == [
        (
            "repair",
            {
                "approval_policy": expected_policy,
                "approved_node_ids": expected_approved,
                "sync_enabled_repos": True,
            },
        )
    ]


def test_maintenance_repair_preserves_baseline_only_policy(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    tmp_path: Path,
) -> None:
    """Baseline-only repair selects the silent baseline policy."""

    _assert_repair_contract(
        monkeypatch,
        capsys,
        tmp_path,
        extra_args=["--baseline-only"],
        expected_policy="silent_baseline_only",
        expected_approved=(),
    )


def test_maintenance_repair_preserves_approved_node_ids(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    tmp_path: Path,
) -> None:
    """Explicit node approvals retain order and choose the approved-id policy."""

    _assert_repair_contract(
        monkeypatch,
        capsys,
        tmp_path,
        extra_args=["--approve", "node.alpha", "--approve", "node.beta"],
        expected_policy="approved_node_ids",
        expected_approved=("node.alpha", "node.beta"),
    )


def test_maintenance_sync_and_check_builds_the_dependency_policy_payload(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    tmp_path: Path,
) -> None:
    """Sync-and-check translates CLI flags into the stable maintenance payload."""

    dependencies = _DependencyCommands({"dependencyReadiness": {"ready": True}})
    monkeypatch.setattr(
        maintenance,
        "build_backend_services",
        lambda *args, **kwargs: _BackendServices(dependencies=dependencies),
    )

    exit_code = maintenance.main(
        [
            "cube-deps",
            "sync-and-check",
            "--workspace",
            str(tmp_path),
            "--baseline-only",
            "--approve",
            "node.alpha",
            "--sync-enabled-repos",
        ]
    )

    assert exit_code == 0
    assert json.loads(capsys.readouterr().out) == {
        "dependencyReadiness": {"ready": True}
    }
    assert dependencies.calls == [
        (
            "sync-and-check",
            {
                "sync": {"mode": "default"},
                "dependencyPolicy": {
                    "includeVersions": True,
                    "baselineOnly": True,
                    "approvedNodeIds": ["node.alpha"],
                    "repair": True,
                },
            },
        )
    ]


def test_maintenance_backend_errors_preserve_status_details_and_diagnostic(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    tmp_path: Path,
) -> None:
    """Expected dependency failures remain machine-readable and actionable."""

    dependencies = _DependencyCommands(
        {},
        failure=BackendError(
            "Approval required",
            status=409,
            details={"nodeIds": ["node.alpha"]},
        ),
    )
    monkeypatch.setattr(
        maintenance,
        "build_backend_services",
        lambda *args, **kwargs: _BackendServices(dependencies=dependencies),
    )

    exit_code = maintenance.main(
        ["cube-deps", "preflight", "--workspace", str(tmp_path)]
    )

    payload = json.loads(capsys.readouterr().out)
    assert exit_code == 1
    assert payload["error"] == "Approval required"
    assert payload["status"] == 409
    assert payload["details"] == {"nodeIds": ["node.alpha"]}
    assert payload["diagnostics"] == [
        {
            "source": "SugarCubes",
            "code": "maintenance_backend_error",
            "severity": "error",
            "title": "SugarCubes maintenance failed",
            "message": "Approval required",
            "details": {"nodeIds": ["node.alpha"]},
        }
    ]


def test_maintenance_treats_non_readiness_payloads_as_success(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    tmp_path: Path,
) -> None:
    """Command payloads without readiness state retain the historical zero exit."""

    dependencies = _DependencyCommands([{"action": "noop"}])
    monkeypatch.setattr(
        maintenance,
        "build_backend_services",
        lambda *args, **kwargs: _BackendServices(dependencies=dependencies),
    )

    exit_code = maintenance.main(
        ["cube-deps", "preflight", "--workspace", str(tmp_path)]
    )

    assert exit_code == 0
    assert json.loads(capsys.readouterr().out) == [{"action": "noop"}]
