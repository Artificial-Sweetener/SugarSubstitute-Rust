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
"""Prove the self-updater manages the pack folder ComfyUI loaded, links included."""

from __future__ import annotations

from pathlib import Path

from sugarcubes.backend.composition import build_backend_services
from sugarcubes.backend.pack_install.ownership import NotOwnedReason
from sugarcubes.backend.self_update.update_state import UpdatePhase
from tests.pack_install.support.install_vectors import make_directory_link
from tests.pack_install.support.pack_fixture import install_registry_pack, pack_files


def test_pack_reached_through_a_link_is_reported_and_left_alone(tmp_path: Path) -> None:
    """A junction-installed pack, like a development link, is never updated in place."""

    checkout = install_registry_pack(tmp_path / "checkout", pack_files("1.0.0"))
    linked = tmp_path / "comfy" / "custom_nodes" / "SugarCubes"
    linked.parent.mkdir(parents=True)
    make_directory_link(linked, checkout)
    services = build_backend_services(linked)
    snapshot = services.self_update.snapshot()
    assert (snapshot.phase, snapshot.not_managed) == (
        UpdatePhase.NOT_MANAGED,
        NotOwnedReason.LINKED_FOLDER,
    )
    assert (
        services.self_update.request_check(force=False).phase is UpdatePhase.NOT_MANAGED
    )
    assert not (checkout / ".sugarcubes" / "updates").exists()


def test_plain_registry_install_is_managed(tmp_path: Path) -> None:
    """The same pack installed as a plain folder is the updater's to manage."""

    pack = install_registry_pack(tmp_path / "comfy", pack_files("1.0.0"))
    snapshot = build_backend_services(pack).self_update.snapshot()
    assert snapshot.phase is UpdatePhase.IDLE
    assert snapshot.not_managed is None
