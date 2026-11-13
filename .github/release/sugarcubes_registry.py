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
"""Build and publish the one SugarCubes registry archive.

``pack`` builds the archive with comfy-cli's own packer, exactly as ``comfy node
pack`` and ``comfy node publish`` do: git-tracked files of the pack folder,
``.comfyignore`` honoured, and ``[tool.comfy] includes`` added. ``publish``
registers the version with the Comfy Registry through comfy-cli and uploads that
same file, so the registry and the GitHub release asset carry identical bytes.

Run from the pack folder (``Sugar/``) with the comfy-cli version pinned by the
release workflow, one command per process: comfy-cli's publish validation reads
``pyproject.toml`` from the working directory at the moment it is first imported.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import sys
import tempfile
import zipfile
from pathlib import Path


def pack(output: Path) -> str:
    """Write the registry archive to ``output`` and return its sha256."""
    from comfy_cli.file_utils import zip_files
    from comfy_cli.registry import extract_node_configuration

    if output.exists():
        raise SystemExit(
            f"{output} already exists; each release builds its archive once."
        )
    config = extract_node_configuration(str(Path.cwd() / "pyproject.toml"))
    if config is None:
        raise SystemExit("pyproject.toml is not a valid Comfy node configuration.")
    includes = list(config.tool_comfy.includes) if config.tool_comfy else []
    output.parent.mkdir(parents=True, exist_ok=True)
    # comfy-cli skips a member named like the archive, so build under a unique name first.
    with tempfile.TemporaryDirectory(dir=output.parent) as folder:
        staged = Path(folder) / f"{output.stem}.building.zip"
        zip_files(str(staged), includes=includes)
        _require_flat_archive(staged)
        staged.replace(output)
    return _sha256(output)


def publish(archive: Path, changelog: str) -> None:
    """Publish this pack version to the Comfy Registry with the exact archive bytes."""
    from comfy_cli.command.custom_nodes.command import validate_node_for_publishing
    from comfy_cli.file_utils import upload_file_to_signed_url
    from comfy_cli.registry import RegistryAPI

    token = os.environ.get("REGISTRY_ACCESS_TOKEN", "")
    if not token:
        raise SystemExit("REGISTRY_ACCESS_TOKEN is not set.")
    # The same validation and security checks `comfy node publish` runs before publishing.
    config = validate_node_for_publishing()
    expected = f"SugarCubes-{config.project.version}.zip"
    if archive.name != expected:
        raise SystemExit(
            f"{archive.name} is not the archive of version {config.project.version}."
        )
    response = RegistryAPI().publish_node_version(config, token, changelog=changelog)
    upload_file_to_signed_url(response.signedUrl, str(archive))
    print(
        f"Published SugarCubes {config.project.version} ({_sha256(archive)}) to the registry"
    )


def _require_flat_archive(path: Path) -> None:
    """The pack root must be the archive root: ComfyUI loads it from there."""
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
    if "__init__.py" not in names or "pyproject.toml" not in names:
        raise SystemExit("The archive does not hold the pack at its root.")


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def main(arguments: list[str]) -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    pack_command = commands.add_parser("pack", help="Build the registry archive.")
    pack_command.add_argument("--output", required=True, type=Path)
    publish_command = commands.add_parser(
        "publish", help="Publish the archive to the registry."
    )
    publish_command.add_argument("--archive", required=True, type=Path)
    publish_command.add_argument("--changelog-file", required=True, type=Path)
    options = parser.parse_args(arguments)
    if options.command == "pack":
        print(f"sha256:{pack(options.output.resolve())}")
    else:
        changelog = options.changelog_file.read_text(encoding="utf-8").strip()
        publish(options.archive.resolve(), changelog)


if __name__ == "__main__":
    main(sys.argv[1:])
