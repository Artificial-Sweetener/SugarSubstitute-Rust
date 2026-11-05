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
"""Compose one host-owned output saving runtime from focused settings, queue and image services."""

from __future__ import annotations

from pathlib import Path
import sys

from ..application.settings_service import OutputSavingSettingsService
from ..application.transfer_service import OutputTransferService
from ..application.media_service import OutputMediaService
from .media_files import HostOutputImageFiles
from .file_revealer import WindowsImageFileRevealer
from .photoshop_opener import WindowsPhotoshopOpener
from .artifact_registry import ManagedArtifactRegistry
from .comfy_node_capabilities import ComfySavingNodeCapabilities
from .managed_image_writer import ManagedImageWriter
from .naming_numbers import OutputNamingNumbers
from .preference_repository import FileOutputSavingPreferences
from .queue_policy import OutputSavingQueuePolicy
from .runtime_images import RuntimeSavingImages
from .settings_paths import HostSavingSettingsPaths
from .source_inventory import ComfySavingSourceInventory
from .ticket_journal import SavingTicketJournal
from .native_mask_bridge import NativeManagedMaskBridge
from .transfer_references import ManagedTransferResolver


class OutputSavingRuntime:
    """Provide the shared saving services without owning their policy or filesystem mechanics."""

    def __init__(
        self,
        *,
        state_root: Path,
        output_root: Path,
        temporary_root: Path,
        input_root: Path | None = None,
    ) -> None:
        """Compose explicit host locations without changing Comfy's configured roots."""
        self.paths = HostSavingSettingsPaths(output_root)
        self.settings = OutputSavingSettingsService(
            FileOutputSavingPreferences(state_root / "preferences.json"), self.paths
        )
        self.journal = SavingTicketJournal(state_root / "tickets.sqlite")
        self.artifacts = ManagedArtifactRegistry(state_root / "artifacts.sqlite")
        self.transfers = OutputTransferService(
            self.settings.load, ManagedTransferResolver(self.artifacts)
        )
        roots = {"output": output_root, "temp": temporary_root}
        if input_root is not None:
            roots["input"] = input_root
        self.media = OutputMediaService(
            self.settings.load,
            HostOutputImageFiles(
                self.artifacts,
                roots,
                WindowsImageFileRevealer(),
                WindowsPhotoshopOpener(),
            ),
        )
        self.mask_uploads = NativeManagedMaskBridge(self.artifacts, temporary_root)
        self.capabilities = capabilities = ComfySavingNodeCapabilities()
        self.inventory = ComfySavingSourceInventory(
            qualified_class=capabilities.qualified_class,
            image_output_class=capabilities.image_output_class,
        )
        self.policy = OutputSavingQueuePolicy(
            settings=self.settings,
            paths=self.paths,
            inventory=self.inventory,
            journal=self.journal,
            metadata_enabled=_metadata_enabled,
        )
        self.images = RuntimeSavingImages(
            self.journal,
            ManagedImageWriter(
                numbers=OutputNamingNumbers(self.journal),
                artifacts=self.artifacts,
                temporary_root=temporary_root,
            ),
        )

    @classmethod
    def from_host(cls) -> OutputSavingRuntime:
        """Validate the running Comfy directory API before granting any path authority."""
        folders = sys.modules.get("folder_paths")
        if folders is None:
            raise RuntimeError(
                "Comfy's folder registry is unavailable for output saving."
            )
        return cls(
            state_root=_directory(folders, "get_user_directory")
            / "__sugarcubes"
            / "output_saving",
            output_root=_directory(folders, "get_output_directory"),
            temporary_root=_directory(folders, "get_temp_directory"),
            input_root=_directory(folders, "get_input_directory"),
        )


def _directory(module: object, name: str) -> Path:
    """Narrow an external host directory into an absolute path at the adapter boundary."""
    method: object = getattr(module, name, None)
    if not callable(method):
        raise RuntimeError(f"Comfy directory API '{name}' is unavailable.")
    value: object = method()
    if not isinstance(value, str) or not value or not Path(value).is_absolute():
        raise RuntimeError(
            f"Comfy directory API '{name}' did not return an absolute path."
        )
    return Path(value)


def _metadata_enabled() -> bool:
    """Honor the server's explicit metadata privacy setting when capturing a queued run."""
    module = sys.modules.get("comfy.cli_args")
    arguments: object = getattr(module, "args", None)
    return getattr(arguments, "disable_metadata", False) is not True
