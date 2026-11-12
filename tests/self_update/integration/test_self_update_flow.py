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
"""Prove SugarCubes checks, stages and installs its own update end to end."""

from __future__ import annotations

from pathlib import Path

import pytest

from sugarcubes.backend.pack_install.ownership import NotOwnedReason
from sugarcubes.backend.pack_install.startup import (
    StartupOutcomeKind,
    run_startup_update,
)
from sugarcubes.backend.self_update import (
    SelfUpdateRejected,
    SelfUpdateService,
    build_self_update_service,
    snapshot_to_wire,
)
from sugarcubes.backend.self_update.http_client import HttpFailureKind
from sugarcubes.backend.self_update.service import RECHECK_INTERVAL_MS
from sugarcubes.backend.self_update.update_state import FailureStage, UpdatePhase
from tests.pack_install.support.pack_fixture import (
    install_registry_pack,
    pack_files,
    snapshot_folder,
)
from tests.self_update.support.fake_release_web import (
    FLAGGED,
    GITHUB_API,
    REGISTRY_API,
    FakeReleaseWeb,
    InlineRunner,
)

OLD = pack_files("1.0.0")
NEW = pack_files("1.1.0", {"sugarcubes/added.py": "added\n"})


class FixedClock:
    """Return a controllable wall-clock time in milliseconds."""

    def __init__(self) -> None:
        """Start at an arbitrary fixed time."""

        self.now = 1_800_000_000_000

    def __call__(self) -> int:
        """Return the current time."""

        return self.now


def _service(
    pack: Path,
    web: FakeReleaseWeb,
    version: str = "1.0.0",
    clock: FixedClock | None = None,
) -> SelfUpdateService:
    return build_self_update_service(
        pack_folder=pack,
        running_version=version,
        http=web,
        clock=clock or FixedClock(),
        runner=InlineRunner(),
    )


def _registry_web(*, new_status: str = "NodeVersionStatusActive") -> FakeReleaseWeb:
    web = FakeReleaseWeb()
    web.publish_registry("1.0.0", OLD, latest=False)
    web.publish_registry("1.1.0", NEW, status=new_status)
    web.publish_empty_github()
    return web


def test_registry_update_is_staged_chosen_installed_and_confirmed(
    tmp_path: Path,
) -> None:
    """The whole flow: check, ready, choose, restart, new version confirms."""

    pack = install_registry_pack(tmp_path, OLD)
    web = _registry_web()
    service = _service(pack, web)
    published: list[int] = []
    service.subscribe(lambda snapshot: published.append(snapshot.revision))
    snapshot = service.request_check(force=False)
    snapshot = service.snapshot()
    assert (snapshot.phase, snapshot.available_version) == (UpdatePhase.READY, "1.1.0")
    assert published == sorted(published) and len(published) >= 3
    assert snapshot_folder(pack)["__init__.py"] == OLD["__init__.py"].encode()
    assert service.schedule_install("1.1.0").phase is UpdatePhase.SCHEDULED
    outcome = run_startup_update(pack)
    assert outcome is not None and outcome.kind is StartupOutcomeKind.INSTALLED
    restarted = _service(pack, web, version="1.1.0")
    restarted.confirm_started()
    wire = snapshot_to_wire(restarted.snapshot())
    assert wire["currentVersion"] == "1.1.0"
    notice = wire["notice"]
    assert isinstance(notice, dict) and notice["kind"] == "installed"
    assert snapshot_folder(pack)["sugarcubes/added.py"] == b"added\n"


def test_flagged_registry_release_falls_back_to_the_github_asset(
    tmp_path: Path,
) -> None:
    """When the Registry lags, the CI-built registry-layout asset is used."""

    pack = install_registry_pack(tmp_path, OLD)
    web = _registry_web(new_status=FLAGGED)
    asset_url = web.publish_github("1.1.0", NEW)
    service = _service(pack, web)
    service.request_check(force=True)
    assert service.snapshot().phase is UpdatePhase.READY
    assert asset_url in web.requests
    assert not any(url.endswith("/1.1.0/node.zip") for url in web.requests)


def test_registry_is_preferred_when_both_publish_the_same_version(
    tmp_path: Path,
) -> None:
    """The Registry comes first; GitHub is only the fallback."""

    pack = install_registry_pack(tmp_path, OLD)
    web = _registry_web()
    asset_url = web.publish_github("1.1.0", NEW)
    _service(pack, web).request_check(force=True)
    assert asset_url not in web.requests


def test_github_asset_with_the_wrong_digest_is_refused(tmp_path: Path) -> None:
    """A downloaded asset must match the SHA-256 GitHub records for it."""

    pack = install_registry_pack(tmp_path, OLD)
    web = _registry_web(new_status=FLAGGED)
    web.publish_github("1.1.0", NEW, digest=lambda _content: "sha256:" + "0" * 64)
    service = _service(pack, web)
    service.request_check(force=True)
    snapshot = service.snapshot()
    assert (snapshot.phase, snapshot.failure) == (
        UpdatePhase.FAILED,
        FailureStage.VERIFY,
    )
    assert not (pack / ".sugarcubes" / "updates" / "staged").exists()


def test_github_asset_without_a_digest_is_refused(tmp_path: Path) -> None:
    """No digest means no way to prove the download; fail closed."""

    pack = install_registry_pack(tmp_path, OLD)
    web = _registry_web(new_status=FLAGGED)
    web.publish_github("1.1.0", NEW, digest=lambda _content: "")
    service = _service(pack, web)
    service.request_check(force=True)
    assert service.snapshot().phase is UpdatePhase.FAILED


def test_offline_check_fails_plainly_and_keeps_the_installed_pack(
    tmp_path: Path,
) -> None:
    """No connection is a typed check failure; nothing changes."""

    pack = install_registry_pack(tmp_path, OLD)
    web = _registry_web()
    web.failures = {"https://": HttpFailureKind.UNREACHABLE}
    original = snapshot_folder(pack)
    service = _service(pack, web)
    service.request_check(force=True)
    snapshot = service.snapshot()
    assert (snapshot.phase, snapshot.failure) == (
        UpdatePhase.FAILED,
        FailureStage.CHECK,
    )
    assert snapshot_folder(pack) == original


def test_one_silent_source_never_reports_up_to_date(tmp_path: Path) -> None:
    """Only a check every source answered may say the pack is current."""

    pack = install_registry_pack(tmp_path, OLD)
    web = _registry_web(new_status=FLAGGED)
    web.failures = {GITHUB_API: HttpFailureKind.TIMED_OUT}
    service = _service(pack, web)
    service.request_check(force=True)
    assert service.snapshot().phase is UpdatePhase.FAILED


def test_local_edit_leaves_the_pack_alone_before_any_new_download(
    tmp_path: Path,
) -> None:
    """A changed tracked file means the folder is not updated, and says so."""

    pack = install_registry_pack(tmp_path, OLD)
    (pack / "sugarcubes" / "core.py").write_text("# my change\n", encoding="utf-8")
    web = _registry_web()
    service = _service(pack, web)
    service.request_check(force=True)
    snapshot = service.snapshot()
    assert (snapshot.phase, snapshot.not_managed) == (
        UpdatePhase.NOT_MANAGED,
        NotOwnedReason.CHANGED_FILES,
    )
    assert not any(url.endswith("/1.1.0/node.zip") for url in web.requests)


def test_git_checkout_is_not_managed_and_never_contacts_the_network(
    tmp_path: Path,
) -> None:
    """A Git checkout is reported at once and left alone."""

    pack = install_registry_pack(tmp_path, OLD)
    (pack / ".git").mkdir()
    web = _registry_web()
    service = _service(pack, web)
    assert service.snapshot().not_managed is NotOwnedReason.GIT_CHECKOUT
    service.request_check(force=False)
    assert web.requests == []


def test_missing_published_copy_of_the_installed_version_fails_closed(
    tmp_path: Path,
) -> None:
    """Without the installed release to compare against, nothing is staged."""

    pack = install_registry_pack(tmp_path, OLD)
    web = FakeReleaseWeb()
    web.publish_registry("1.1.0", NEW)
    web.publish_empty_github()
    service = _service(pack, web)
    service.request_check(force=True)
    assert service.snapshot().phase is UpdatePhase.FAILED


def test_recent_check_is_reused_until_the_user_asks_again(tmp_path: Path) -> None:
    """Automatic checks are coalesced; the user's check always runs."""

    pack = install_registry_pack(tmp_path, OLD)
    web = FakeReleaseWeb()
    web.publish_registry("1.0.0", OLD)
    web.publish_empty_github()
    clock = FixedClock()
    service = _service(pack, web, clock=clock)
    service.request_check(force=False)
    assert service.snapshot().phase is UpdatePhase.UP_TO_DATE
    requests = len(web.requests)
    service.request_check(force=False)
    assert len(web.requests) == requests
    clock.now += RECHECK_INTERVAL_MS
    service.request_check(force=False)
    assert len(web.requests) > requests
    requests = len(web.requests)
    service.request_check(force=True)
    assert len(web.requests) > requests


def test_choice_is_refused_for_a_version_that_is_not_ready(tmp_path: Path) -> None:
    """Install accepts only the release that is staged now."""

    pack = install_registry_pack(tmp_path, OLD)
    service = _service(pack, _registry_web())
    with pytest.raises(SelfUpdateRejected):
        service.schedule_install("1.1.0")
    service.request_check(force=True)
    with pytest.raises(SelfUpdateRejected):
        service.schedule_install("9.9.9")
    service.schedule_install("1.1.0")
    assert service.cancel_install().phase is UpdatePhase.READY


def test_failed_install_at_start_runs_the_previous_version_with_a_report(
    tmp_path: Path,
) -> None:
    """A start that cannot install restores the backup and the entry reports it."""

    pack = install_registry_pack(tmp_path, OLD)
    web = _registry_web()
    service = _service(pack, web)
    service.request_check(force=True)
    service.schedule_install("1.1.0")
    original = snapshot_folder(pack)
    run_startup_update(pack)
    rolled_back = run_startup_update(pack)
    assert rolled_back is not None and rolled_back.kind is StartupOutcomeKind.RESTORED
    assert snapshot_folder(pack) == original
    wire = snapshot_to_wire(_service(pack, web).snapshot())
    notice = wire["notice"]
    assert isinstance(notice, dict)
    assert (notice["kind"], notice["reason"]) == ("restored", "did-not-start")
    assert "previous version" in str(notice["message"])


def test_registry_descriptor_for_another_pack_is_rejected(tmp_path: Path) -> None:
    """Metadata naming another pack is invalid and never used."""

    pack = install_registry_pack(tmp_path, OLD)
    web = _registry_web()
    web.json[REGISTRY_API] = {**_descriptor(web), "node_id": "OtherPack"}
    service = _service(pack, web)
    service.request_check(force=True)
    assert service.snapshot().phase is UpdatePhase.FAILED


def _descriptor(web: FakeReleaseWeb) -> dict[str, object]:
    value = web.json[REGISTRY_API]
    assert isinstance(value, dict)
    return dict(value)
