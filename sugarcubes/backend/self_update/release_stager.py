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
"""Check for a newer SugarCubes and stage it, without touching the installed pack.

Order of work: prove the folder is a registry install this updater owns; find the
newest release (Registry first, GitHub release asset when the Registry lags);
prove no tracked file was changed locally by comparing it with the published
release of the installed version; then download, verify and stage the new
release. Any failure leaves the installed pack and any earlier staged release as
they were.
"""

from __future__ import annotations

import logging
import secrets
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

from ..pack_install.archive import extract_registry_archive
from ..pack_install.errors import PackInstallError
from ..pack_install.file_lock import exclusive_update_lock
from ..pack_install.filesystem import remove_tree
from ..pack_install.ownership import (
    NotOwnedPack,
    NotOwnedReason,
    OwnedPack,
    changed_tracked_files,
    inspect_pack_folder,
    installed_file_digests,
)
from ..pack_install.pack_identity import ReleaseVersion
from ..pack_install.staging import MISSING_FILE_DIGEST, StagedStore, StagingRequest
from ..pack_install.startup import updates_root
from .http_client import Cancelled, HttpClient, HttpFailure
from .release_sources import (
    MAX_ARCHIVE_BYTES,
    GitHubReleases,
    RegistryReleases,
    ReleaseMetadataError,
    ReleaseOffer,
)
from .update_state import FailureStage

_logger = logging.getLogger("sugarcubes.self_update")


@dataclass(frozen=True, slots=True)
class NotManaged:
    """The folder is not one this updater may replace."""

    current_version: str
    reason: NotOwnedReason


@dataclass(frozen=True, slots=True)
class UpToDate:
    """No newer release is published."""

    current_version: str


@dataclass(frozen=True, slots=True)
class Staged:
    """A newer release is verified and waiting to be installed."""

    current_version: str
    version: str


@dataclass(frozen=True, slots=True)
class StageFailed:
    """A step failed; the installed pack is unchanged."""

    current_version: str
    stage: FailureStage
    cause: str


StageResult = NotManaged | UpToDate | Staged | StageFailed
ProgressCallback = Callable[[str], None]


class _StepFailure(Exception):
    """Carry a failed step out of the staging sequence."""

    def __init__(self, stage: FailureStage, cause: str) -> None:
        """Keep the stage and a diagnostic cause."""

        super().__init__(cause)
        self.stage = stage
        self.cause = cause


class ReleaseStager:
    """Own one check-and-stage attempt for the pack folder ComfyUI loaded."""

    def __init__(
        self,
        *,
        pack_folder: Path,
        pack_id: str,
        registry: RegistryReleases,
        github: GitHubReleases,
        http: HttpClient,
    ) -> None:
        """Bind the attempt to its folder and release sources."""

        self._pack_folder = pack_folder
        self._pack_id = pack_id
        self._registry = registry
        self._github = github
        self._http = http
        self._root = updates_root(pack_folder)

    def ownership(self) -> OwnedPack | NotOwnedPack:
        """Classify the installed folder without network access."""

        ownership = inspect_pack_folder(self._pack_folder)
        if (
            isinstance(ownership, OwnedPack)
            and ownership.identity.name.casefold() != self._pack_id.casefold()
        ):
            return NotOwnedPack(
                self._pack_folder,
                NotOwnedReason.NOT_REGISTRY_INSTALL,
                "The folder holds another pack.",
            )
        return ownership

    def check_and_stage(
        self,
        *,
        installed_version: str,
        cancelled: Cancelled,
        on_downloading: ProgressCallback,
    ) -> StageResult:
        """Run one attempt and return its typed result; never raises for I/O."""

        ownership = self.ownership()
        if isinstance(ownership, NotOwnedPack):
            return NotManaged(installed_version, ownership.reason)
        current = str(ownership.identity.version)
        try:
            with exclusive_update_lock(self._root):
                return self._stage_newest(ownership, cancelled, on_downloading)
        except _StepFailure as failure:
            _logger.warning(
                "SugarCubes update event=stage_failed stage=%s cause=%s",
                failure.stage.value,
                failure.cause,
            )
            return StageFailed(current, failure.stage, failure.cause)
        except (OSError, PackInstallError) as exc:
            _logger.warning(
                "SugarCubes update event=stage_failed stage=check cause=%s", exc
            )
            return StageFailed(current, FailureStage.CHECK, type(exc).__name__)

    def _stage_newest(
        self,
        pack: OwnedPack,
        cancelled: Cancelled,
        on_downloading: ProgressCallback,
    ) -> StageResult:
        """Find, verify and stage the newest release while holding the lock."""

        current = pack.identity.version
        offer = self._newest_offer(current)
        store = StagedStore(self._root)
        if offer is None:
            store.discard()
            return UpToDate(str(current))
        installed = installed_file_digests(pack)
        if _already_staged(store, offer, current, installed):
            return Staged(str(current), str(offer.version))
        published = self._published_digests(current, cancelled)
        if changed_tracked_files(installed, published):
            return NotManaged(str(current), NotOwnedReason.CHANGED_FILES)
        on_downloading(str(offer.version))
        work = self._root / f"download-{secrets.token_hex(6)}"
        work.mkdir(parents=True)
        try:
            archive = work / "release.zip"
            digest = self._download(offer, archive, cancelled)
            try:
                store.stage(
                    StagingRequest(
                        archive_path=archive,
                        pack_id=self._pack_id,
                        version=str(offer.version),
                        from_version=str(current),
                        source=offer.source.value,
                        archive_sha256=digest,
                        installed_digests=installed,
                    )
                )
            except PackInstallError as exc:
                raise _StepFailure(FailureStage.VERIFY, exc.code.value) from exc
        finally:
            remove_tree(work)
        _logger.info(
            "SugarCubes update event=staged version=%s from=%s source=%s",
            offer.version,
            current,
            offer.source.value,
        )
        return Staged(str(current), str(offer.version))

    def _newest_offer(self, current: ReleaseVersion) -> ReleaseOffer | None:
        """Return the newest release above `current`, preferring the Registry."""

        offers: list[ReleaseOffer] = []
        failures = 0
        for name, find in (
            ("registry", self._registry.latest),
            ("github", self._github.latest),
        ):
            try:
                offer = find()
            except (HttpFailure, ReleaseMetadataError) as exc:
                failures += 1
                _logger.warning(
                    "SugarCubes update event=source_unavailable source=%s cause=%s",
                    name,
                    exc,
                )
                continue
            if offer is not None and offer.version > current:
                offers.append(offer)
        if not offers and failures:
            # Only a check every source answered may call the pack up to date.
            raise _StepFailure(FailureStage.CHECK, "A release source did not answer.")
        if not offers:
            return None
        # `max` keeps the first of equal versions, and the Registry is listed first.
        return max(offers, key=lambda offer: offer.version)

    def _published_digests(
        self, version: ReleaseVersion, cancelled: Cancelled
    ) -> dict[str, str]:
        """Return each file digest of the published release of `version`."""

        offer: ReleaseOffer | None = None
        finders: tuple[Callable[[], ReleaseOffer | None], ...] = (
            lambda: self._registry.exact(version, require_active=False),
            lambda: self._github.exact(version),
        )
        for find in finders:
            try:
                offer = find()
            except (HttpFailure, ReleaseMetadataError) as exc:
                _logger.warning(
                    "SugarCubes update event=baseline_source_unavailable cause=%s", exc
                )
                continue
            if offer is not None:
                break
        if offer is None:
            raise _StepFailure(
                FailureStage.CHECK,
                "The installed release could not be found to compare.",
            )
        work = self._root / f"baseline-{secrets.token_hex(6)}"
        work.mkdir(parents=True)
        try:
            archive = work / "installed.zip"
            self._download(offer, archive, cancelled)
            try:
                return extract_registry_archive(archive, work / "files").file_digests
            except PackInstallError as exc:
                raise _StepFailure(FailureStage.VERIFY, exc.code.value) from exc
        finally:
            remove_tree(work)

    def _download(self, offer: ReleaseOffer, target: Path, cancelled: Cancelled) -> str:
        """Download one offer and prove its digest when the source publishes one."""

        try:
            digest = self._http.download(
                offer.archive_url,
                target,
                allowed_hosts=offer.allowed_hosts,
                max_bytes=MAX_ARCHIVE_BYTES,
                cancelled=cancelled,
            )
        except HttpFailure as exc:
            raise _StepFailure(FailureStage.DOWNLOAD, exc.kind.value) from exc
        if offer.sha256 is not None and digest != offer.sha256:
            raise _StepFailure(FailureStage.VERIFY, "digest-mismatch")
        return digest


def _already_staged(
    store: StagedStore,
    offer: ReleaseOffer,
    current: ReleaseVersion,
    installed: dict[str, str | None],
) -> bool:
    """Return whether the offered release is already staged against these files."""

    try:
        staged = store.load()
        if (
            staged is None
            or staged.version != str(offer.version)
            or staged.from_version != str(current)
            or staged.installed_digests
            != {
                path: digest or MISSING_FILE_DIGEST
                for path, digest in installed.items()
            }
        ):
            return False
        store.verify(staged)
    except PackInstallError:
        store.discard()
        return False
    return True


__all__ = [
    "NotManaged",
    "ReleaseStager",
    "StageFailed",
    "StageResult",
    "Staged",
    "UpToDate",
]
