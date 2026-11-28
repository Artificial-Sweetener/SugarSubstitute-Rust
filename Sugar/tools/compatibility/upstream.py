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
"""Locate SugarCubes' upstream dependencies in a shipped ComfyUI core and frontend."""

from __future__ import annotations

import ast
import hashlib
import json
import re
import subprocess
from collections import defaultdict
from pathlib import Path

from tools.compatibility.inventory import SUPPORT


def frontend_names(entry: str) -> set[str]:
    """Resolve literal native selectors, component names, chunk capabilities and host identifiers."""
    category, _, value = entry.partition(" ")
    if entry.startswith("host selector or token "):
        value = entry.removeprefix("host selector or token ")
        if value.startswith("["):
            match = re.fullmatch(r'\[data-testid=["\']([\w.-]+)["\']\]', value)
            return {match[1]} if match else set()
        return {value.lstrip(".#")}
    if entry.startswith(("host member ", "host adapter member ", "typed host member ")):
        match = re.search(r"(?:\.|\?\.)([A-Za-z_$][\w$]*)$", entry)
        return {match[1]} if match else set()
    if entry.startswith(("dynamic member ", "native component ")):
        return {entry.rsplit(" ", 1)[-1]}
    if entry.startswith("bundle discovery "):
        name = entry.removeprefix("bundle discovery ").strip("'\u0022")
        return {name} if name.isidentifier() else set()
    if category == "DOM":
        match = re.fullmatch(r'lookup ["\']([\w-]+)["\']', value)
        return {match[1]} if match else set()
    return set()


def source_symbols(
    directory: Path, wanted: set[str], anchors: set[str]
) -> tuple[dict[str, list[str]], dict[str, dict[str, str]]]:
    """Find referring frontend sources in the exact installed release's source maps."""
    found: dict[str, set[str]] = defaultdict(set)
    definitions: dict[str, dict[str, str]] = {}
    seen: set[tuple[str, int]] = set()
    mappings = sorted(directory.glob("**/*.js.map")) + sorted(
        directory.glob("**/*.css.map")
    )
    if not mappings:
        raise ValueError("The installed frontend has no source maps")
    for mapping in mappings:
        document = json.loads(mapping.read_bytes())
        for name, source in zip(
            document.get("sources", []), document.get("sourcesContent", [])
        ):
            if not source:
                continue
            if "node_modules/" in name:
                package_path = name.rsplit("node_modules/", 1)[-1]
                if not package_path.startswith(
                    (
                        "@comfyorg/",
                        "@vue/",
                        "vue/",
                        "primevue/",
                        "@primevue/",
                        "reka-ui/",
                        "@primeuix/",
                    )
                ):
                    continue
                name = "node_modules/" + package_path
            key = (name, hash(source))
            if key in seen:
                continue
            seen.add(key)
            terms = wanted.intersection(re.findall(r"[\w$-]+", source))
            for word in terms:
                found[word].add(name)
            matched = sorted(terms & anchors)
            if matched:
                definitions[name] = {
                    "sha256": hashlib.sha256(source.encode()).hexdigest(),
                    "anchors": ", ".join(matched),
                }
    return {key: sorted(paths) for key, paths in sorted(found.items())}, definitions


def python_definitions(
    core: Path, entries: dict[str, list[str]]
) -> tuple[dict[str, str], dict[str, dict[str, object]]]:
    """Locate consumed Python callables, their signatures, implementations and SugarCubes owners."""
    requested: dict[str, set[str]] = defaultdict(set)
    consumers: dict[str, set[str]] = defaultdict(set)
    interfaces = {
        "PromptServerLike": "server",
        "PromptQueueLike": "execution",
        "ComfyExecutionModuleLike": "execution",
        "StartupFolderRegistry": "folder_paths",
    }
    for path, values in entries.items():
        if not path.endswith(".py"):
            continue
        imports: set[str] = set()
        attributes: set[str] = set()
        for value in values:
            if value.startswith("dynamic host import "):
                imports.add(value.removeprefix("dynamic host import "))
            elif value.startswith("host import "):
                name = value.removeprefix("host import ")
                if name.startswith("from "):
                    declaration = ast.parse(name).body[0]
                    if isinstance(declaration, ast.ImportFrom) and declaration.module:
                        imports.add(declaration.module)
                        for alias in declaration.names:
                            requested[declaration.module].add(alias.name)
                            consumers[f"{declaration.module}.{alias.name}"].add(path)
                            imports.add(f"{declaration.module}.{alias.name}")
                else:
                    imports.add(name)
            elif value.startswith("host member "):
                attributes.add(value.removeprefix("host member ").split(".")[-1])
            elif value.startswith("dynamic attribute "):
                call = re.match(
                    r"""(?:getattr|hasattr|setattr)\([^,]+, ['"]([\w]+)['"]""",
                    value.removeprefix("dynamic attribute "),
                )
                if call:
                    attributes.add(call[1])
            elif value.startswith("host protocol "):
                member = re.match(r"host protocol (\w+)\.(\w+)", value)
                if member and member[1] in interfaces:
                    requested[interfaces[member[1]]].add(member[2])
                    consumers[f"{interfaces[member[1]]}.{member[2]}"].add(path)
        for module in imports:
            requested[module].update(attributes)
            for attribute in attributes:
                consumers[f"{module}.{attribute}"].add(path)
    signatures: dict[str, str] = {}
    definitions: dict[str, dict[str, object]] = {}
    for module, attributes in sorted(requested.items()):
        source = core.joinpath(*module.split(".")).with_suffix(".py")
        if not source.is_file():
            source = source.with_suffix("") / "__init__.py"
        if not source.is_file():
            continue
        signatures[module] = "module"
        tree = ast.parse(source.read_text(encoding="utf-8-sig"))

        def record(
            qualified: str, node: ast.FunctionDef | ast.AsyncFunctionDef
        ) -> None:
            """Ignore source locations and formatting when comparing executable implementations."""
            signatures[qualified] = ast.unparse(node.args)
            definitions[qualified] = {
                "file": source.relative_to(core).as_posix(),
                "line": node.lineno,
                "sha256": hashlib.sha256(ast.dump(node).encode()).hexdigest(),
                "sugarSources": sorted(consumers[f"{module}.{node.name}"]),
            }

        for function in tree.body:
            if isinstance(function, ast.ClassDef) and function.name in {
                "PromptServer",
                "PromptQueue",
            }:
                for method in function.body:
                    if (
                        isinstance(method, (ast.FunctionDef, ast.AsyncFunctionDef))
                        and method.name in attributes
                    ):
                        record(f"{module}.{function.name}.{method.name}", method)
            if (
                isinstance(function, (ast.FunctionDef, ast.AsyncFunctionDef))
                and function.name in attributes
            ):
                record(f"{module}.{function.name}", function)
    return signatures, definitions


def record_dependencies(checkout: Path, python: Path, output: Path) -> None:
    """Retain upstream definitions alongside runtime evidence for source-level diagnosis."""
    entries = json.loads(
        (SUPPORT / "host-dependencies.json").read_text(encoding="utf-8")
    )
    names = {
        name
        for path, values in entries.items()
        if path.endswith((".ts", ".tsx"))
        for entry in values
        for name in frontend_names(entry)
    }
    package = subprocess.check_output(
        [
            str(python),
            "-c",
            "import importlib.util as u; print(u.find_spec('comfyui_frontend_package').submodule_search_locations[0])",
        ],
        text=True,
    ).strip()
    consumers: dict[str, set[str]] = defaultdict(set)
    anchors: set[str] = set()
    for path, values in entries.items():
        if not path.endswith((".ts", ".tsx")):
            continue
        for entry in values:
            for symbol in frontend_names(entry):
                consumers[symbol].add(path)
                if (
                    entry.startswith(
                        (
                            "native component ",
                            "bundle discovery ",
                            "host selector or token ",
                        )
                    )
                    and len(symbol) >= 6
                ):
                    anchors.add(symbol)
    symbols, definitions = source_symbols(Path(package) / "static", names, anchors)
    signatures, core_definitions = python_definitions(checkout, entries)
    result = {
        "frontend": symbols,
        "definitions": definitions,
        "consumers": {key: sorted(paths) for key, paths in consumers.items()},
        "core": signatures,
        "coreDefinitions": core_definitions,
    }
    (output / "upstream.json").write_text(
        json.dumps(result, indent=2), encoding="utf-8"
    )
