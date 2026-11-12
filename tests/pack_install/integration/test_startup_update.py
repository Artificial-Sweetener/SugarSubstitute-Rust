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
"""Prove the startup step installs only a chosen, unchanged update and recovers."""

from __future__ import annotations

from pathlib import Path

import pytest

from sugarcubes.backend.pack_install.errors import PackInstallError
from sugarcubes.backend.pack_install.file_lock import exclusive_update_lock
from sugarcubes.backend.pack_install.staging import StagedStore
from sugarcubes.backend.pack_install.startup import (
    StartupOutcomeKind,
    confirm_started,
    read_outcome,
    run_startup_update,
    updates_root,
)
from tests.pack_install.support.pack_fixture import (
    FaultInjector,
    SimulatedPowerCut,
    install_registry_pack,
    pack_files,
    snapshot_folder,
    stage_release,
)

USER_DATA = {".sugarcubes/library/mine.cube": "my cube", ".env": "TOKEN=kept"}
OLD = pack_files("1.0.0", {"sugarcubes/dropped.py": "dropped in 1.1.0\n"})
NEW = pack_files("1.1.0", {"sugarcubes/added.py": "added in 1.1.0\n"})


def _prepare(tmp_path: Path, *, choose: bool = True) -> tuple[Path, StagedStore]:
    pack = install_registry_pack(tmp_path, OLD)
    for relative, text in USER_DATA.items():
        destination = pack.joinpath(*relative.split("/"))
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(text, encoding="utf-8")
    store = stage_release(tmp_path, pack, NEW, "1.1.0")
    release = store.load()
    assert release is not None
    if choose:
        store.schedule(release)
    return pack, store


def _expected_new(pack: Path) -> dict[str, bytes]:
    files = {path: text.encode("utf-8") for path, text in {**NEW, **USER_DATA}.items()}
    files[".tracking"] = (pack / ".tracking").read_bytes()
    files[".tracking-digests.json"] = (pack / ".tracking-digests.json").read_bytes()
    return files


def test_chosen_update_installs_at_start_and_is_confirmed(tmp_path: Path) -> None:
    """The release replaces only tracked files; user data stays; start confirms it."""

    pack, store = _prepare(tmp_path)
    outcome = run_startup_update(pack)
    assert outcome is not None and outcome.kind is StartupOutcomeKind.INSTALLED
    assert snapshot_folder(pack) == _expected_new(pack)
    assert (pack / ".tracking").read_text(encoding="utf-8").splitlines() == list(NEW)
    assert store.load() is None
    confirmed = confirm_started(pack, "1.1.0")
    assert confirmed is not None and confirmed.reason == ""
    assert not (updates_root(pack) / "transaction").exists()
    assert run_startup_update(pack) is None
    assert snapshot_folder(pack) == _expected_new(pack)


def test_update_that_never_confirms_is_rolled_back_at_the_next_start(
    tmp_path: Path,
) -> None:
    """A new version whose backend never came up is replaced by the previous one."""

    pack, _store = _prepare(tmp_path)
    original = snapshot_folder(pack)
    run_startup_update(pack)
    outcome = run_startup_update(pack)
    assert outcome is not None
    assert (outcome.kind, outcome.reason) == (
        StartupOutcomeKind.RESTORED,
        "did-not-start",
    )
    assert snapshot_folder(pack) == original
    assert read_outcome(updates_root(pack)) == outcome


def test_confirmation_from_another_version_settles_nothing(tmp_path: Path) -> None:
    """Only the version that was installed may confirm its own start."""

    pack, _store = _prepare(tmp_path)
    run_startup_update(pack)
    assert confirm_started(pack, "1.0.0") is None
    assert (updates_root(pack) / "transaction").exists()


def test_staged_release_without_the_users_choice_is_not_installed(
    tmp_path: Path,
) -> None:
    """Staging never implies installing."""

    pack, _store = _prepare(tmp_path, choose=False)
    original = snapshot_folder(pack)
    assert run_startup_update(pack) is None
    assert snapshot_folder(pack) == original


@pytest.mark.parametrize(
    ("change", "reason"),
    [
        ("edit-tracked", "changed-files"),
        ("manager-updated", "target-changed"),
        ("staged-corrupted", "staged-corrupt"),
        ("became-git", "git-checkout"),
    ],
)
def test_changes_after_staging_refuse_the_install(
    tmp_path: Path, change: str, reason: str
) -> None:
    """Anything that changed since staging leaves the installed pack untouched."""

    pack, _store = _prepare(tmp_path)
    if change == "edit-tracked":
        (pack / "sugarcubes" / "core.py").write_text("# my edit\n", encoding="utf-8")
    elif change == "manager-updated":
        (pack / "pyproject.toml").write_text(
            '[project]\nname = "SugarCubes"\nversion = "1.0.5"\n', encoding="utf-8"
        )
    elif change == "staged-corrupted":
        staged_file = updates_root(pack) / "staged" / "files" / "__init__.py"
        staged_file.write_text("tampered\n", encoding="utf-8")
    else:
        (pack / ".git").mkdir()
    original = snapshot_folder(pack)
    outcome = run_startup_update(pack)
    assert outcome is not None
    assert (outcome.kind, outcome.reason) == (StartupOutcomeKind.NOT_INSTALLED, reason)
    assert snapshot_folder(pack) == original
    assert run_startup_update(pack) is None


def test_busy_folder_is_reported_without_touching_the_pack(tmp_path: Path) -> None:
    """A second process updating the same folder gets a typed busy outcome."""

    pack, _store = _prepare(tmp_path)
    original = snapshot_folder(pack)
    with exclusive_update_lock(updates_root(pack)):
        with pytest.raises(PackInstallError) as raised:
            run_startup_update(pack)
    assert raised.value.code.value == "busy"
    assert snapshot_folder(pack) == original


def _count_operations(tmp_path: Path) -> int:
    pack, _store = _prepare(tmp_path / "count")
    injector = FaultInjector()
    run_startup_update(pack, injector.operations())
    return injector.calls


def test_failure_at_every_write_restores_the_previous_pack(tmp_path: Path) -> None:
    """An error at any single write rolls the pack back before ComfyUI imports it."""

    total = _count_operations(tmp_path)
    assert total > 10
    for step in range(1, total + 1):
        pack, _store = _prepare(tmp_path / f"fail-{step}")
        original = snapshot_folder(pack)
        outcome = run_startup_update(pack, FaultInjector(fail_at=step).operations())
        assert outcome is not None, step
        assert outcome.kind is StartupOutcomeKind.NOT_INSTALLED, step
        assert snapshot_folder(pack) == original, step
        assert not (updates_root(pack) / "transaction").exists(), step


def test_power_cut_at_every_write_recovers_at_the_next_start(tmp_path: Path) -> None:
    """A crash at any write leaves a pack the next start restores or completes."""

    total = _count_operations(tmp_path)
    for step in range(1, total + 1):
        pack, _store = _prepare(tmp_path / f"cut-{step}")
        original = snapshot_folder(pack)
        with pytest.raises(SimulatedPowerCut):
            run_startup_update(pack, FaultInjector(crash_at=step).operations())
        outcome = run_startup_update(pack)
        assert outcome is not None, step
        if outcome.kind is StartupOutcomeKind.RESTORED:
            assert outcome.reason == "interrupted", step
            assert snapshot_folder(pack) == original, step
            assert run_startup_update(pack) is None, step
        else:
            assert outcome.kind is StartupOutcomeKind.INSTALLED, step
            assert snapshot_folder(pack) == _expected_new(pack), step


def test_power_cut_during_rollback_is_finished_by_the_next_start(
    tmp_path: Path,
) -> None:
    """Rollback itself is restart-safe: repeating it reaches the same pack."""

    pack, _store = _prepare(tmp_path / "count")
    original = snapshot_folder(pack)
    run_startup_update(pack)
    counter = FaultInjector()
    run_startup_update(pack, counter.operations())
    rollback_writes = counter.calls
    assert rollback_writes > 3
    for step in range(1, rollback_writes + 1):
        pack, _store = _prepare(tmp_path / f"rollback-{step}")
        original = snapshot_folder(pack)
        run_startup_update(pack)
        with pytest.raises(SimulatedPowerCut):
            run_startup_update(pack, FaultInjector(crash_at=step).operations())
        outcome = run_startup_update(pack)
        assert outcome is not None and outcome.kind is StartupOutcomeKind.RESTORED, step
        assert snapshot_folder(pack) == original, step
