# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Build reviewed capture sources using explicit immutable local inputs, without running them."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from repair_ninja import repair_output
from verify_source import verify

VS = Path(r"C:\Program Files\Microsoft Visual Studio\2022\Community")
MSVC = "14.43.34808"
SDK = "10.0.26100.0"
GN_SHA = "fa111680baf435088415ebedf4cdd41d07db8651c482d944c841e0f7d46ca580"
NINJA_SHA = "5e25e8fa872204e831e19e1eb73dbb7802ca97d339b89b22ae2ec0d0fb5fb352"


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def verify_environment(output, temporary):
    block = (output / "environment.amd64").read_text(encoding="utf-8").rstrip("\0")
    environment = dict(pair.split("=", 1) for pair in block.split("\0"))
    for variable in ("INCLUDE", "LIB"):
        if MSVC not in environment[variable] or SDK not in environment[variable]:
            raise ValueError("Generated compiler environment escaped the reviewed versions")
    for variable in ("TEMP", "TMP"):
        if Path(environment[variable]).resolve() != temporary.resolve():
            raise ValueError("Generated compiler temporary output escaped D: artifacts")
    if str(VS / "VC/Tools/MSVC" / MSVC / "bin/Hostx64/x64").lower() not in environment["PATH"].lower():
        raise ValueError("Generated compiler executable path escaped the reviewed instance")


def run(source, artifacts, tools, jobs):
    recipe_names = ("build_native.py", "prepare_source.py", "verify_source.py", "repair_ninja.py")
    recipe_hashes = {name: digest(Path(__file__).with_name(name)) for name in recipe_names}
    if sys.version_info[:2] != (3, 12):
        raise ValueError("The reviewed build requires explicit Python 3.12")
    if source.resolve().drive.upper() != "E:" or artifacts.resolve().drive.upper() != "D:":
        raise ValueError("Source must remain on E: and all generated build output on D:")
    if digest(tools / "gn.exe") != GN_SHA or digest(tools / "ninja.exe") != NINJA_SHA:
        raise ValueError("Immutable GN/Ninja binary mismatch")
    compiler = VS / "VC/Tools/MSVC" / MSVC / "bin/Hostx64/x64/cl.exe"
    if not compiler.is_file():
        raise ValueError("Reviewed MSVC compiler is absent")
    vswhere = Path(os.environ["ProgramFiles(x86)"]) / "Microsoft Visual Studio/Installer/vswhere.exe"
    instances = json.loads(subprocess.check_output([
        str(vswhere), "-products", "Microsoft.VisualStudio.Product.Community",
        "-version", "[17.13,17.14)", "-format", "json", "-utf8"], text=True, encoding="utf-8"))
    matching = [item for item in instances if Path(item["installationPath"]) == VS]
    if len(matching) != 1 or matching[0]["catalog"]["productDisplayVersion"] != "17.13.6":
        raise ValueError("Reviewed Visual Studio instance/version mismatch")
    temporary = artifacts / "temp"
    temporary.mkdir(parents=True, exist_ok=True)
    output = artifacts / "out-msvc-x64"
    output.mkdir(exist_ok=True)
    authored = Path(__file__).resolve().parents[2] / "native/crashpad"
    before_inventory = artifacts / "source-inventory-before.json"
    verify(source, authored, artifacts / "downloads", before_inventory)
    before_bytes = before_inventory.read_bytes()
    env = dict(os.environ, TEMP=str(temporary), TMP=str(temporary), PYTHONDONTWRITEBYTECODE="1", SUGAR_CRASHPAD_VS=str(VS),
               PATH=str(tools) + os.pathsep + os.environ["PATH"])
    arguments = 'is_debug=false target_cpu="x64" mini_chromium_is_clang=false win_toolchain_path="' + VS.as_posix() + '"'
    commands = [[str(tools / "gn.exe"), "gen", str(output), "--root=" + str(source),
                 "--root-target=//sugarsubstitute", "--script-executable=" + sys.executable, "--args=" + arguments],
                [str(tools / "ninja.exe"), "-C", str(output), "-j", str(jobs), "capture_bundle"]]
    for index, command in enumerate(commands):
        if index == 1:
            verify_environment(output, temporary)
            repair_output(source, output, artifacts / "generated-ninja-repair.json")
        with (artifacts / f"build-{index}.log").open("w", encoding="utf-8") as log:
            result = subprocess.run(command, cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT)
        if result.returncode:
            raise RuntimeError(f"Reviewed native build step {index} failed ({result.returncode})")
    verify_environment(output, temporary)
    after_inventory = artifacts / "source-inventory-after.json"
    verify(source, authored, artifacts / "downloads", after_inventory)
    if before_bytes != after_inventory.read_bytes():
        raise ValueError("Reviewed build inputs changed during compilation")
    if recipe_hashes != {name: digest(Path(__file__).with_name(name)) for name in recipe_names}:
        raise ValueError("Reviewed build recipe changed during compilation")
    files = [output / name for name in ("crashpad_handler.exe", "crashpad_handler.pdb",
                                        "sugarsubstitute_crashpad_client.dll", "sugarsubstitute_crashpad_client.pdb")]
    if not all(path.is_file() for path in files):
        raise ValueError("Exact capture runtime and matching symbol outputs are incomplete")
    receipt = {"commands": commands, "python": sys.executable, "visual_studio": matching[0]["catalog"]["productDisplayVersion"],
               "msvc": MSVC, "sdk": SDK, "gn_sha256": GN_SHA, "ninja_sha256": NINJA_SHA,
               "source_inventory_before_sha256": digest(before_inventory),
               "source_inventory_after_sha256": digest(after_inventory),
               "recipe_sha256": recipe_hashes,
               "runtime_executed": False, "files": [{"name": path.name, "bytes": path.stat().st_size, "sha256": digest(path)} for path in sorted(files)]}
    (artifacts / "build-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--artifacts", required=True, type=Path)
    parser.add_argument("--tools", required=True, type=Path)
    parser.add_argument("--jobs", type=int, default=4, choices=range(1, 9))
    args = parser.parse_args()
    run(args.source, args.artifacts, args.tools, args.jobs)
