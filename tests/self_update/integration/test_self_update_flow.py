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
    REGISTRY_VERSIONS,
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
    web.publish_registry("1.0.0", OLD)
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
    restarted.accept_backend_start(nodes_registered=True)
    wire = snapshot_to_wire(restarted.snapshot())
    assert wire["currentVersion"] == "1.1.0"
    notice = wire["notice"]
    assert isinstance(notice, dict) and notice["kind"] == "installed"
    assert snapshot_folder(pack)["sugarcubes/added.py"] == b"added\n"


def _old_only_web() -> FakeReleaseWeb:
    """Publish only the installed release on the Registry."""

    web = FakeReleaseWeb()
    web.publish_registry("1.0.0", OLD)
    return web


def _registry_archive(version: str) -> str:
    return f"/SugarCubes/{version}/node.zip"


@pytest.mark.parametrize(
    ("status", "installable"),
    [
        ("NodeVersionStatusActive", True),
        (FLAGGED, True),
        ("NodeVersionStatusDeleted", False),
        ("NodeVersionStatusBanned", False),
        ("NodeVersionStatusPending", False),
    ],
)
def test_registry_status_decides_only_whether_a_release_is_still_offered(
    tmp_path: Path, status: str, installable: bool
) -> None:
    """Flagged installs like Active; removed or unreviewed releases are skipped."""

    pack = install_registry_pack(tmp_path, OLD)
    web = _registry_web(new_status=status)
    service = _service(pack, web)
    service.request_check(force=True)
    snapshot = service.snapshot()
    if installable:
        assert (snapshot.phase, snapshot.available_version) == (
            UpdatePhase.READY,
            "1.1.0",
        )
        assert any(url.endswith(_registry_archive("1.1.0")) for url in web.requests)
    else:
        assert snapshot.phase is UpdatePhase.UP_TO_DATE
        assert not any(url.endswith(_registry_archive("1.1.0")) for url in web.requests)


def test_registry_behind_github_installs_the_github_asset(tmp_path: Path) -> None:
    """The newest version across both sources wins, wherever it is published."""

    pack = install_registry_pack(tmp_path, OLD)
    web = _registry_web()
    asset_url = web.publish_github("1.2.0", pack_files("1.2.0"))
    service = _service(pack, web)
    service.request_check(force=True)
    assert service.snapshot().available_version == "1.2.0"
    assert asset_url in web.requests


def test_registry_ahead_of_github_installs_from_the_registry(tmp_path: Path) -> None:
    """A newer Registry release is used even when GitHub has an older one."""

    pack = install_registry_pack(tmp_path, OLD)
    web = _old_only_web()
    web.publish_registry("1.2.0", pack_files("1.2.0"))
    github_asset = web.publish_github("1.1.0", NEW)
    service = _service(pack, web)
    service.request_check(force=True)
    assert service.snapshot().available_version == "1.2.0"
    assert github_asset not in web.requests


def test_registry_is_preferred_when_both_publish_the_same_version(
    tmp_path: Path,
) -> None:
    """The same version in both places comes from the Registry."""

    pack = install_registry_pack(tmp_path, OLD)
    web = _registry_web()
    asset_url = web.publish_github("1.1.0", NEW)
    _service(pack, web).request_check(force=True)
    assert asset_url not in web.requests


@pytest.mark.parametrize(
    ("failing", "unchecked", "publish_on"),
    [
        (REGISTRY_VERSIONS, ("registry",), "github"),
        (GITHUB_API, ("github-release",), "registry"),
    ],
)
def test_one_unreachable_source_still_finds_the_update_and_says_so(
    tmp_path: Path, failing: str, unchecked: tuple[str, ...], publish_on: str
) -> None:
    """A source that cannot be reached does not hide the other's update."""

    pack = install_registry_pack(tmp_path, OLD)
    web = _old_only_web()
    if publish_on == "github":
        web.publish_github("1.1.0", NEW)
    else:
        web.publish_registry("1.1.0", NEW)
        web.publish_empty_github()
    web.failures = {failing: HttpFailureKind.TIMED_OUT}
    service = _service(pack, web)
    service.request_check(force=True)
    snapshot = service.snapshot()
    assert (snapshot.phase, snapshot.available_version) == (UpdatePhase.READY, "1.1.0")
    assert snapshot.unchecked_sources == unchecked
    wire = snapshot_to_wire(snapshot)
    notes = wire["uncheckedSources"]
    assert isinstance(notes, list) and notes[0]["source"] == unchecked[0]


def test_unreachable_source_with_nothing_newer_elsewhere_is_not_up_to_date(
    tmp_path: Path,
) -> None:
    """Only a check both sources answered may call the pack up to date."""

    pack = install_registry_pack(tmp_path, OLD)
    web = _old_only_web()
    web.failures = {GITHUB_API: HttpFailureKind.TIMED_OUT}
    service = _service(pack, web)
    service.request_check(force=True)
    snapshot = service.snapshot()
    assert (snapshot.phase, snapshot.failure) == (
        UpdatePhase.FAILED,
        FailureStage.CHECK,
    )
    assert snapshot.unchecked_sources == ("github-release",)


def test_github_asset_with_the_wrong_digest_is_refused(tmp_path: Path) -> None:
    """A downloaded asset must match the SHA-256 GitHub records for it."""

    pack = install_registry_pack(tmp_path, OLD)
    web = _old_only_web()
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
    web = _old_only_web()
    web.publish_github("1.1.0", NEW, digest=lambda _content: "")
    service = _service(pack, web)
    service.request_check(force=True)
    assert service.snapshot().phase is UpdatePhase.FAILED


def test_offline_check_fails_plainly_and_keeps_the_installed_pack(
    tmp_path: Path,
) -> None:
    """No connection at all is a typed check failure naming both sources."""

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
    assert snapshot.unchecked_sources == ("registry", "github-release")
    assert snapshot_folder(pack) == original


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


def test_registry_version_list_for_another_pack_is_rejected(tmp_path: Path) -> None:
    """A version record naming another pack invalidates the Registry's answer."""

    pack = install_registry_pack(tmp_path, OLD)
    web = _registry_web()
    listing = web.json[REGISTRY_VERSIONS]
    assert isinstance(listing, list)
    listing.append({**listing[0], "node_id": "OtherPack"})
    service = _service(pack, web)
    service.request_check(force=True)
    snapshot = service.snapshot()
    assert snapshot.phase is UpdatePhase.FAILED
    assert snapshot.unchecked_sources == ("registry",)


def _baseline_requests(web: FakeReleaseWeb, version: str) -> list[str]:
    return [
        url
        for url in web.requests
        if f"version={version}" in url or url.endswith(f"/{version}/node.zip")
    ]


def test_manager_install_is_compared_once_then_checked_offline(tmp_path: Path) -> None:
    """A Manager install gets a digest record after one release comparison."""

    pack = install_registry_pack(tmp_path, OLD)
    assert not (pack / ".tracking-digests.json").exists()
    web = _registry_web()
    service = _service(pack, web)
    service.request_check(force=True)
    assert _baseline_requests(web, "1.0.0")
    assert (pack / ".tracking-digests.json").is_file()
    offline = _registry_web()
    offline.failures = {f"{REGISTRY_API}?version=1.0.0": HttpFailureKind.UNREACHABLE}
    (pack / "sugarcubes" / "core.py").write_text("# edited later\n", encoding="utf-8")
    edited = _service(pack, offline)
    edited.request_check(force=True)
    assert edited.snapshot().not_managed is NotOwnedReason.CHANGED_FILES
    assert not _baseline_requests(offline, "1.0.0")


def test_pack_installed_by_the_updater_needs_no_network_to_find_edits(
    tmp_path: Path,
) -> None:
    """An install made by this format carries its record from the first start."""

    pack = install_registry_pack(tmp_path, OLD)
    web = _registry_web()
    service = _service(pack, web)
    service.request_check(force=True)
    service.schedule_install("1.1.0")
    run_startup_update(pack)
    started = _service(pack, web, version="1.1.0")
    started.accept_backend_start(nodes_registered=True)
    started.report_page_load(loaded=True, nodes_registered=True)
    newer = FakeReleaseWeb()
    newer.publish_registry("1.2.0", pack_files("1.2.0"))
    newer.publish_empty_github()
    (pack / "sugarcubes" / "added.py").write_text("# my change\n", encoding="utf-8")
    checker = _service(pack, newer, version="1.1.0")
    checker.request_check(force=True)
    assert checker.snapshot().not_managed is NotOwnedReason.CHANGED_FILES
    assert not _baseline_requests(newer, "1.1.0")


def test_record_left_by_an_earlier_install_is_ignored_after_manager_updates(
    tmp_path: Path,
) -> None:
    """When another installer changes the pack, the stale record is not trusted."""

    pack = install_registry_pack(tmp_path, OLD)
    web = _registry_web()
    _service(pack, web).request_check(force=True)
    assert (pack / ".tracking-digests.json").is_file()
    manager = install_registry_pack(tmp_path / "manager", pack_files("1.0.5"))
    for name in (
        "pyproject.toml",
        "__init__.py",
        "sugarcubes/core.py",
        "web/app.js",
        ".tracking",
    ):
        (pack / name).write_bytes((manager / name).read_bytes())
    web.publish_registry("1.0.5", pack_files("1.0.5"))
    checker = _service(pack, web, version="1.0.5")
    checker.request_check(force=True)
    assert checker.snapshot().phase is UpdatePhase.READY
    assert _baseline_requests(web, "1.0.5")
