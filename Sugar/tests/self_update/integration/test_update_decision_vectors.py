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
"""Prove the updater's decisions satisfy every shared language-neutral decision vector.

The Substitute host installs SugarCubes with its own implementation of the same
format and decisions and reads this vector file in place, so these groups are
driven end to end through the real updater against a faked HTTP boundary.
"""

from __future__ import annotations

import hashlib
from pathlib import Path
from typing import cast

import pytest

from sugarcubes.backend.pack_install.digest_manifest import write_digest_manifest
from sugarcubes.backend.pack_install.entry_paths import EntryName
from sugarcubes.backend.pack_install.pack_identity import read_pack_identity
from sugarcubes.backend.pack_install.replacement import FileOperations
from sugarcubes.backend.pack_install.staging import StagedStore
from sugarcubes.backend.pack_install.tracking import read_tracking
from sugarcubes.backend.pack_install.startup import (
    StartupOutcome,
    StartupOutcomeKind,
    run_startup_update,
    updates_root,
)
from sugarcubes.backend.self_update import build_self_update_service
from sugarcubes.backend.self_update.http_client import HttpFailureKind
from sugarcubes.backend.self_update.release_sources import (
    GitHubReleases,
    RegistryReleases,
)
from sugarcubes.backend.self_update.release_stager import (
    NotManaged,
    ReleaseStager,
    Staged,
    StageFailed,
    StageResult,
    UpToDate,
)
from tests.pack_install.support.install_vectors import (
    JsonObject,
    build_folder,
    drive,
)
from tests.pack_install.support.pack_fixture import (
    SimulatedPowerCut,
    install_registry_pack,
    pack_files,
    stage_release,
)
from tests.self_update.support.fake_release_web import (
    CONFIG,
    GITHUB_API,
    REGISTRY_VERSIONS,
    FakeReleaseWeb,
    InlineRunner,
)

_GITHUB_LIST = f"{GITHUB_API}/releases?per_page=50"


def _expect(vector: JsonObject) -> JsonObject:
    return cast(JsonObject, vector["expect"])


def _stager(pack: Path, web: FakeReleaseWeb) -> ReleaseStager:
    return ReleaseStager(
        pack_folder=pack,
        pack_id=CONFIG.pack_id,
        registry=RegistryReleases(
            pack_id=CONFIG.pack_id, config=CONFIG.registry, http=web
        ),
        github=GitHubReleases(config=CONFIG.github_release, http=web),
        http=web,
    )


def _check(pack: Path, web: FakeReleaseWeb, installed: str) -> StageResult:
    return _stager(pack, web).check_and_stage(
        installed_version=installed,
        cancelled=lambda: False,
        on_downloading=lambda _version: None,
    )


def _record_digests(pack: Path, version: str) -> None:
    """Write the record this format leaves beside `.tracking` at install time."""

    entries = read_tracking(pack)
    assert entries is not None
    write_digest_manifest(
        pack,
        version=version,
        files={
            entry.path: hashlib.sha256(
                pack.joinpath(*entry.path.split("/")).read_bytes()
            ).hexdigest()
            for entry in entries
            if not entry.is_directory
        },
    )


def _publish_answers(web: FakeReleaseWeb, vector: JsonObject) -> None:
    """Publish what each source answers, or make it fail to answer."""

    registry = cast(JsonObject, vector["registry"])
    if registry.get("unavailable"):
        web.failures[REGISTRY_VERSIONS] = HttpFailureKind.UNREACHABLE
    else:
        web.publish_empty_registry()
        # Publishing prepends, so publish oldest-listed last to keep listed order.
        for item in reversed(cast(list[JsonObject], registry["versions"])):
            version = str(item["version"])
            web.publish_registry(
                version, pack_files(version), status=str(item["status"])
            )
    github = cast(JsonObject, vector["github"])
    if github.get("unavailable"):
        web.failures[_GITHUB_LIST] = HttpFailureKind.UNREACHABLE
        return
    web.publish_empty_github()
    listing = cast(list[JsonObject], web.json[_GITHUB_LIST])
    for item in cast(list[JsonObject], github["releases"]):
        web.publish_github(str(item["version"]), pack_files(str(item["version"])))
        listing[-1]["draft"] = bool(item.get("draft", False))
        listing[-1]["prerelease"] = bool(item.get("prerelease", False))


@drive("sourceSelection")
def test_source_selection_vectors(vector: JsonObject, tmp_path: Path) -> None:
    """Both sources are asked; the newest installable version wins, registry first."""

    installed = str(vector["installed"])
    pack = install_registry_pack(tmp_path, pack_files(installed))
    _record_digests(pack, installed)
    web = FakeReleaseWeb()
    _publish_answers(web, vector)
    result = _check(pack, web, installed)
    expected = _expect(vector)
    assert [kind.value for kind in result.unchecked_sources] == expected["unchecked"]
    outcome = expected["outcome"]
    if outcome == "staged":
        assert isinstance(result, Staged)
        assert result.version == expected["version"]
        staged = StagedStore(updates_root(pack)).load()
        assert staged is not None and staged.source == expected["source"]
    elif outcome == "up-to-date":
        assert isinstance(result, UpToDate)
    else:
        assert outcome == "check-failed"
        assert isinstance(result, StageFailed) and result.stage.value == "check"


def _with_version(files: dict[str, str], version: str) -> dict[str, str]:
    return {
        **files,
        "pyproject.toml": f'[project]\nname = "SugarCubes"\nversion = "{version}"\n',
    }


@drive("updateRefusals")
def test_update_refusal_vectors(vector: JsonObject, tmp_path: Path) -> None:
    """Only an unchanged registry install is updated; every refusal says why."""

    folder = cast(JsonObject, vector["installed"])
    files = cast(dict[str, str], folder["files"])
    pack = build_folder(tmp_path, folder)
    if vector["digestRecord"]:
        _record_digests(pack, "1.0.0")
    for relative, text in cast(dict[str, str | None], vector["edits"]).items():
        target = pack.joinpath(*relative.split("/"))
        if text is None:
            target.unlink()
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(text.encode("utf-8"))
    newer = str(vector["newer"])
    web = FakeReleaseWeb()
    web.publish_empty_github()
    if not vector["digestRecord"]:
        web.publish_registry("1.0.0", files)
    web.publish_registry(newer, _with_version(files, newer))
    result = _check(pack, web, "1.0.0")
    expected = _expect(vector)
    if expected["outcome"] == "staged":
        assert isinstance(result, Staged) and result.version == expected["version"]
        return
    assert expected["outcome"] == "not-managed"
    assert isinstance(result, NotManaged)
    assert result.reason.value == expected["reason"]
    assert StagedStore(updates_root(pack)).load() is None


def _power_cut_operations() -> FileOperations:
    """Crash when the new ownership record is written, mid-replacement."""

    def write(folder: Path, entries: tuple[EntryName, ...]) -> None:
        raise SimulatedPowerCut("write_tracking")

    return FileOperations(write_tracking=write)


@drive("rollbackTriggers")
def test_rollback_trigger_vectors(vector: JsonObject, tmp_path: Path) -> None:
    """Only a failure of the new version itself restores the previous one."""

    pack = install_registry_pack(tmp_path, pack_files("1.0.0"))
    store = stage_release(tmp_path, pack, pack_files("1.1.0"), "1.1.0")
    release = store.load()
    assert release is not None
    store.schedule(release)
    events = cast(list[str], vector["events"])
    if events[0] == "power-cut-while-replacing":
        with pytest.raises(SimulatedPowerCut):
            run_startup_update(pack, _power_cut_operations())
        events = events[1:]
    else:
        installed = run_startup_update(pack)
        assert installed is not None and installed.kind is StartupOutcomeKind.INSTALLED
    service = build_self_update_service(
        pack_folder=pack,
        running_version="1.1.0",
        http=FakeReleaseWeb(),
        clock=lambda: 0,
        runner=InlineRunner(),
    )
    last: StartupOutcome | None = None
    for event in events:
        if event == "backend-ready":
            service.accept_backend_start(nodes_registered=True)
        elif event == "backend-ready-nodes-missing":
            service.accept_backend_start(nodes_registered=False)
        elif event == "backend-start-unobservable":
            service.start_unobservable()
        elif event == "page-loaded":
            service.report_page_load(loaded=True, nodes_registered=True)
        elif event == "page-loaded-nodes-missing":
            service.report_page_load(loaded=True, nodes_registered=False)
        elif event == "page-failed":
            service.report_page_load(loaded=False, nodes_registered=True)
        else:
            assert event == "restart", f"unknown rollback event {event}"
            last = run_startup_update(pack)
    expected = _expect(vector)
    assert str(read_pack_identity(pack).version) == expected["version"]
    backup_held = (updates_root(pack) / "transaction").exists()
    verdict = expected["verdict"]
    if verdict == "restored":
        assert last is not None and last.kind is StartupOutcomeKind.RESTORED
        assert last.reason == expected["reason"]
        assert not backup_held
    else:
        assert backup_held is (verdict == "pending")
        assert verdict in {"kept", "pending"}
