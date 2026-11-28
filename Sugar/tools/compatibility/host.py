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
"""Run the release compatibility probes in an isolated ComfyUI host."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from contextlib import contextmanager
from pathlib import Path
from typing import Iterator
from urllib.error import URLError
from urllib.request import urlopen

SUGAR = Path(__file__).resolve().parents[2]


def git(root: Path, *arguments: str) -> str:
    """Read the source identity or Git's list of distributable source files."""
    return subprocess.check_output(
        ["git", "-C", str(root), *arguments], encoding="utf-8"
    )


def copy_checkout(
    source: Path, target: Path, *, working: bool = False
) -> dict[str, str]:
    """Copy repository files without personal settings, other installed packs or databases."""
    options = ["--others", "--exclude-standard"] if working else []
    files = git(source, "ls-files", "--cached", *options, "-z", "--", ".")
    digests = {kind: hashlib.sha256() for kind in ("product", "suite")}
    for name in sorted(set(files.split("\x00")) - {""}):
        original = source / name
        if original.is_file():
            destination = target / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            if working:
                content = original.read_bytes()
                destination.write_bytes(content)
                kind = (
                    "suite"
                    if name.startswith(("tests/", "scripts/", "tools/"))
                    else "product"
                )
                digests[kind].update(
                    name.encode() + b"\0" + hashlib.sha256(content).digest()
                )
            else:
                shutil.copyfile(original, destination)
    return (
        {kind: digest.hexdigest() for kind, digest in digests.items()}
        if working
        else {}
    )


def loopback_port() -> int:
    """Select a local TCP port for this private Comfy instance."""
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return int(listener.getsockname()[1])


@contextmanager
def running_host(
    checkout: Path,
    python: Path,
    scratch: Path,
    evidence: Path,
    frontend_root: Path | None = None,
) -> Iterator[str]:
    """Wait for this specific CPU host, preserving its log through probe or setup failures."""
    core = scratch / "comfy"
    base = scratch / "profile"
    copy_checkout(checkout, core)
    # The saved installation preference must take effect before Comfy registers loaders.
    from sugarcubes_model_roots.store import ModelRootStore

    model_root = scratch / "compatibility-models"
    ModelRootStore(base).save(model_root)
    checkpoints = model_root / "checkpoints"
    checkpoints.mkdir(parents=True, exist_ok=True)
    header = json.dumps(
        {"sample": {"dtype": "F32", "shape": [1], "data_offsets": [0, 4]}}
    ).encode()
    header += b" " * (-len(header) % 8)
    (checkpoints / "compatibility-catalog-only.safetensors").write_bytes(
        len(header).to_bytes(8, "little") + header + bytes(4)
    )
    pack = base / "custom_nodes" / "SugarCubes"
    digests = copy_checkout(SUGAR, pack, working=True)
    (evidence / "pack-source.json").write_text(
        json.dumps(digests, indent=2), encoding="utf-8"
    )
    catalog = pack / ".sugarcubes" / "Artificial-Sweetener" / "Base-Cubes"
    catalog.mkdir(parents=True)
    for fixture in (SUGAR / "tests/comfy_contract/support").glob("*.cube"):
        shutil.copyfile(fixture, catalog / fixture.name)
    observer = base / "custom_nodes" / "SugarCompatibilityObserver"
    observer.mkdir()
    shutil.copyfile(
        SUGAR / "tests/comfy_contract/support/host_observer.py",
        observer / "__init__.py",
    )
    port = loopback_port()
    address = f"http://127.0.0.1:{port}"
    args = [
        str(python),
        "-u",
        str(core / "main.py"),
        "--cpu",
        "--listen",
        "127.0.0.1",
        "--database-url",
        f"sqlite:///{(base / 'comfy.db').as_posix()}",
        "--port",
        str(port),
        "--base-directory",
        str(base),
        "--disable-auto-launch",
        "--disable-all-custom-nodes",
        "--whitelist-custom-nodes",
        "SugarCubes",
        "SugarCompatibilityObserver",
    ]
    if frontend_root:
        args.extend(["--front-end-root", str(frontend_root.resolve())])
    with (evidence / "comfy.log").open("w", encoding="utf-8") as log:
        environment = {
            **os.environ,
            "TEMP": str(scratch),
            "TMP": str(scratch),
            "TMPDIR": str(scratch),
        }
        for secret in ("GH_TOKEN", "GITHUB_TOKEN", "ACTIONS_ID_TOKEN_REQUEST_TOKEN"):
            environment.pop(secret, None)
        process = subprocess.Popen(
            args, cwd=core, stdout=log, stderr=subprocess.STDOUT, env=environment
        )
        try:
            deadline = time.monotonic() + 600
            retry_interval = 0.1
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise RuntimeError(
                        f"Host exited with {process.returncode}; see comfy.log"
                    )
                try:
                    with urlopen(
                        f"{address}/system_stats",
                        timeout=min(2.0, max(0.1, deadline - time.monotonic())),
                    ) as response:
                        stats = json.load(response)
                    if "system" in stats:
                        break
                except (URLError, TimeoutError):
                    pass
                try:
                    process.wait(timeout=retry_interval)
                except subprocess.TimeoutExpired:
                    retry_interval = min(retry_interval * 2, 1.0)
            else:
                raise TimeoutError(
                    "Host did not publish system_stats within 600 seconds"
                )
            yield address
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait()


def main() -> int:
    """Run the same normal entry point against a development checkout or release."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--comfy", type=Path, required=True)
    parser.add_argument("--python", type=Path, required=True)
    parser.add_argument(
        "--frontend-root", type=Path, help="Serve an explicitly supplied built frontend"
    )
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--test-timeout",
        type=int,
        default=3600,
        help="Maximum total seconds for browser scenarios",
    )
    parser.add_argument("playwright_args", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    checkout, python = args.comfy.resolve(), args.python.resolve()
    frontend = subprocess.check_output(
        [
            str(python),
            "-c",
            "from importlib.metadata import version;print(version('comfyui-frontend-package'))",
        ],
        text=True,
    ).strip()
    identity = {
        "core": git(checkout, "rev-parse", "HEAD").strip(),
        "frontend": frontend,
        "sugar": git(SUGAR, "rev-parse", "HEAD").strip(),
    }
    identity["sugar_status"] = git(SUGAR, "status", "--short", "--", ".")
    identity["core_status"] = git(checkout, "status", "--short", "--untracked-files=no")
    if args.frontend_root:
        identity["frontend_override_index_sha256"] = hashlib.sha256(
            (args.frontend_root / "index.html").read_bytes()
        ).hexdigest()
    control = json.loads(
        (SUGAR / "tests/comfy_contract/support/control.json").read_text(
            encoding="utf-8"
        )
    )
    edits_reference = any(
        argument == "-u" or argument.startswith("--update-snapshots")
        for argument in args.playwright_args
    )
    if edits_reference and (
        args.frontend_root
        or identity["core_status"]
        or any(identity[k] != v for k, v in control.items())
    ):
        parser.error(
            "Reference updates require the unmodified reviewed control checkout and its pinned frontend"
        )
    packages = subprocess.check_output(
        [
            str(python),
            "-c",
            "from importlib.metadata import distributions;print('\\n'.join(sorted(str(d.metadata['Name'])+'=='+d.version for d in distributions())))",
        ],
        text=True,
    )
    (output / "packages.txt").write_text(packages, encoding="utf-8")
    (output / "versions.json").write_text(
        json.dumps(identity, indent=2), encoding="utf-8"
    )
    from tools.compatibility.upstream import record_dependencies

    try:
        record_dependencies(checkout, python, output)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        (output / "upstream.json").write_text(
            json.dumps({"frontend": {}, "core": {}, "error": str(error)}),
            encoding="utf-8",
        )
    with tempfile.TemporaryDirectory(dir=output.parent, prefix="comfy-") as temporary:
        with running_host(
            checkout, python, Path(temporary), output, args.frontend_root
        ) as address:
            test_args = list(args.playwright_args)
            if test_args[:1] == ["--"]:
                test_args.pop(0)
            result = subprocess.run(
                [
                    "node",
                    "node_modules/@playwright/test/cli.js",
                    "test",
                    "--config",
                    "scripts/compatibility/playwright.config.ts",
                    *test_args,
                ],
                cwd=SUGAR,
                env={
                    **os.environ,
                    "SUGAR_COMFY_URL": address,
                    "SUGAR_COMPAT_MEASURE": "1",
                    "SUGAR_COMPAT_OUTPUT": str(output),
                    "TEMP": temporary,
                    "TMP": temporary,
                    "TMPDIR": temporary,
                },
                timeout=args.test_timeout,
                check=False,
            )
            from tools.compatibility.coverage import record_coverage

            record_coverage(output)
            return result.returncode


if __name__ == "__main__":
    sys.exit(main())
