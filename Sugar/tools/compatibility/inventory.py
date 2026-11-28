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
"""Index first-party host assumptions with their source locations and behavioral owners."""

from __future__ import annotations

import argparse
import ast
import json
import os
import re
import subprocess
import tomllib
from collections import defaultdict
from dataclasses import asdict, dataclass
from pathlib import Path

from tools.architecture.scanner import visible_repository_paths
from tools.testing.policy import load_test_policy

ROOT = Path(__file__).resolve().parents[2]
SUPPORT = ROOT / "tests" / "comfy_contract" / "support"
HOST_MODULES = {
    "folder_paths",
    "comfy",
    "comfy_api",
    "comfy_execution",
    "comfy_extras",
    "server",
    "nodes",
    "execution",
    "node_helpers",
    "latent_preview",
    "comfy_api_nodes",
    "app",
}
HOST_NAMES = {
    "PromptServer",
    "ExecutionBlocker",
    "folder_paths",
    "prompt_queue",
    "prompt_server",
    "_prompt_server",
    "comfy_execution",
    "_execution",
    "node_replace_manager",
    "NODE_CLASS_MAPPINGS",
    "folder_names_and_paths",
}


@dataclass(frozen=True, order=True, slots=True)
class Touch:
    """Keep the source location independently of its line-number-insensitive review identity."""

    path: str
    line: int
    kind: str
    assumption: str
    intent: str = ""


def backend(path: str) -> list[Touch]:
    """Follow imported host names and record dynamic module discovery and native adapter protocols."""
    source = (ROOT / path).read_text(encoding="utf-8-sig")
    tree = ast.parse(source)
    result: list[Touch] = []
    names = set(HOST_NAMES)
    for node in ast.walk(tree):
        if (
            isinstance(node, ast.ImportFrom)
            and not node.level
            and (node.module or "").split(".")[0] in HOST_MODULES
        ):
            names.update(alias.asname or alias.name for alias in node.names)
            result.append(Touch(path, node.lineno, "host import", ast.unparse(node)))
        if isinstance(node, ast.Import):
            for alias in node.names:
                if alias.name.split(".")[0] in HOST_MODULES:
                    names.add(alias.asname or alias.name.split(".")[0])
                    result.append(Touch(path, node.lineno, "host import", alias.name))
        if (
            isinstance(node, ast.Call)
            and node.args
            and isinstance(node.args[0], ast.Constant)
        ):
            value = node.args[0].value
            if isinstance(value, str) and value.split(".")[0] in HOST_MODULES:
                if ast.unparse(node.func) in {
                    "import_module",
                    "importlib.import_module",
                    "sys.modules.get",
                    "__import__",
                }:
                    result.append(
                        Touch(path, node.lineno, "dynamic host import", value)
                    )
    touches_host = (
        bool(result)
        or "comfy" in Path(path).stem
        or any(
            isinstance(node, ast.arg) and node.arg in names for node in ast.walk(tree)
        )
    )
    for node in ast.walk(tree):
        if (
            touches_host
            and isinstance(node, ast.ClassDef)
            and any(
                ast.unparse(base).split(".")[-1] == "Protocol" for base in node.bases
            )
        ):
            for member in node.body:
                if isinstance(member, (ast.FunctionDef, ast.AsyncFunctionDef)):
                    result.append(
                        Touch(
                            path,
                            member.lineno,
                            "host protocol",
                            f"{node.name}.{member.name}({ast.unparse(member.args)})",
                        )
                    )
                elif isinstance(member, ast.AnnAssign):
                    result.append(
                        Touch(
                            path,
                            member.lineno,
                            "host protocol",
                            f"{node.name}.{ast.unparse(member)}",
                        )
                    )
        if isinstance(node, ast.Attribute):
            parts = ast.unparse(node).split(".")
            if any(p in names for p in parts[:-1]):
                result.append(
                    Touch(path, node.lineno, "host member", ast.unparse(node))
                )
        if (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and node.func.id in {"getattr", "setattr", "hasattr"}
        ):
            if (
                len(node.args) >= 2
                and isinstance(node.args[1], ast.Constant)
                and isinstance(node.args[1].value, str)
            ):
                if touches_host:
                    result.append(
                        Touch(
                            path,
                            node.lineno,
                            "dynamic attribute",
                            ast.unparse(node)[:260],
                        )
                    )
    documentation: dict[int, str] = {}
    for owner in ast.walk(tree):
        if isinstance(
            owner, (ast.Module, ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef)
        ):
            description = ast.get_docstring(owner)
            if description:
                start = getattr(owner, "lineno", 1)
                end = getattr(owner, "end_lineno", len(source.splitlines()))
                for line in range(start, (end or start) + 1):
                    documentation[line] = description.split("\n\n")[0]
    return [
        Touch(
            item.path,
            item.line,
            item.kind,
            item.assumption,
            documentation.get(item.line, ""),
        )
        for item in result
    ]


def native_edges(path: str) -> list[Touch]:
    """Locate wire dependencies in native editor clients and host-facing stylesheet selectors."""
    entries = []
    for line, source in enumerate(
        (ROOT / path).read_text(encoding="utf-8-sig").splitlines(), 1
    ):
        if path.endswith(".rs"):
            for match in re.finditer(
                r'/(?:system_stats|object_info|userdata|upload|view|history|queue|prompt|sugarcubes/[^\s"?#]+)\b',
                source,
            ):
                entries.append(
                    Touch(
                        path,
                        line,
                        "native wire route",
                        match[0],
                        "Native editor client request to the Comfy server.",
                    )
                )
            if path.endswith("/connection.rs") and (
                member := re.search(r'\.get\("([\w]+)"\)', source)
            ):
                entries.append(
                    Touch(
                        path,
                        line,
                        "readiness member",
                        member[1],
                        "Validate the ComfyUI system_stats response before attaching the editor guest.",
                    )
                )
        else:
            for match in re.finditer(
                r"(?:[.#](?:comfy|lg-|p-|graph-canvas)[\w-]*|--(?:comfy|p-)[\w-]+)",
                source,
            ):
                entries.append(
                    Touch(
                        path, line, "host selector or token", match[0], source.strip()
                    )
                )
    return entries


def discover() -> list[Touch]:
    """Scan authored sources, including newly created adapters in the worktree."""
    entries: list[Touch] = []
    paths = source_scope()
    for path in paths:
        if path.endswith(".py"):
            entries.extend(backend(path))
        elif path.endswith((".rs", ".css")):
            entries.extend(native_edges(path))
    output = subprocess.check_output(
        ["node", "--import", "tsx", "scripts/compatibility/frontend-inventory.ts"],
        input=json.dumps(
            [
                p
                for p in paths
                if p.startswith("integrations/") and p.endswith((".ts", ".tsx"))
            ]
        ),
        cwd=ROOT,
        encoding="utf-8",
    )
    entries.extend(Touch(**item) for item in json.loads(output))
    return sorted(set(entries))


def assertion_sources(paths: set[str]) -> dict[str, list[str]]:
    """Find local characterizations importing each production module without assuming behavioral coverage."""
    lookup: dict[str, set[str]] = defaultdict(set)
    modules = {
        path.removesuffix(".py").replace("/", "."): path
        for path in paths
        if path.endswith(".py")
    }
    tests = ROOT / "tests"
    for test in (*tests.rglob("*.test.ts"), *tests.rglob("test_*.py")):
        source = test.read_text(encoding="utf-8-sig")
        relative = test.relative_to(ROOT).as_posix()
        if test.suffix == ".py":
            for node in ast.walk(ast.parse(source)):
                if isinstance(node, ast.ImportFrom) and node.module in modules:
                    lookup[modules[node.module]].add(relative)
                elif isinstance(node, ast.Import):
                    for alias in node.names:
                        if alias.name in modules:
                            lookup[modules[alias.name]].add(relative)
        else:
            for specifier in re.findall(
                r"""(?:from\s*|import\(\s*)['"](\.[^'"]+)['"]""", source
            ):
                target = (test.parent / specifier).resolve().with_suffix(".ts")
                if target.is_relative_to(ROOT):
                    candidate = target.relative_to(ROOT).as_posix()
                    if candidate in paths:
                        lookup[candidate].add(relative)
    return {path: sorted(files) for path, files in lookup.items()}


def source_scope() -> list[str]:
    """Require an explicit inventory decision for every new first-party runtime source."""
    extensions = {
        ".py",
        ".ts",
        ".tsx",
        ".rs",
        ".css",
        ".html",
        ".js",
        ".jsx",
        ".mjs",
        ".cjs",
        ".mts",
        ".cts",
        ".vue",
    }
    excluded = (
        "scripts/",
        "tests/",
        "tools/",
        "third_party/",
        "web/",
        "stubs/",
        "frontend/static/third_party/",
    )
    names = visible_repository_paths(ROOT)
    if names is None:
        # The staged gate exports the exact index without .git and attaches node_modules.
        exported: set[str] = set()
        generated = {"__pycache__", "build", "coverage", "node_modules", "target"}
        for directory, children, files in os.walk(ROOT):
            base = Path(directory).relative_to(ROOT)
            children[:] = [
                name
                for name in children
                if name not in generated
                and not name.startswith(".")
                and not ((base / name).as_posix() + "/").startswith(excluded)
                and not (Path(directory) / name).is_junction()
            ]
            exported.update((base / name).as_posix() for name in files)
        names = frozenset(exported)
    return sorted(
        name
        for name in names
        if Path(name).suffix in extensions
        and not name.startswith(excluded)
        and (ROOT / name).is_file()
    )


def api_status(entry: Touch) -> str:
    """Distinguish documented callback names from internal data read inside those callbacks."""
    if entry.kind == "extension hook" and entry.assumption in {
        "beforeRegisterNodeDef",
        "nodeCreated",
        "loadedGraphNode",
        "setup",
        "beforeConfigureGraph",
        "afterConfigureGraph",
        "registerCustomNodes",
    }:
        return "Documented callback: https://docs.comfy.org/custom-nodes/js/javascript_hooks"
    if entry.kind == "host selector or token":
        return "Frontend presentation implementation detail"
    if entry.kind in {
        "host member",
        "typed host member",
        "host adapter member",
        "dynamic attribute",
        "dynamic member",
    }:
        return "Native member or structural assumption. No documented stability guarantee recorded."
    if entry.kind in {"bundle discovery", "native component"}:
        return "Private frontend bundle or renderer implementation"
    if entry.kind in {
        "native wire route",
        "host transport",
        "readiness member",
        "module, endpoint or setting",
    }:
        return "Host transport, module or setting. The consumed payload and timing require compatibility checks."
    if entry.kind.endswith("host import"):
        return "Imported ComfyUI implementation. Its local call sites define the required behavior."
    return (
        "Host implementation dependency without a verified public stability guarantee"
    )


def main() -> None:
    """Emit a searchable source ledger or compare it with the reviewed inventory snapshot."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--write-index", action="store_true")
    args = parser.parse_args()
    entries = discover()
    assumptions: dict[str, list[str]] = defaultdict(list)
    for entry in entries:
        assumptions[entry.path].append(f"{entry.kind} {entry.assumption}")
    index = {path: sorted(set(values)) for path, values in sorted(assumptions.items())}
    path = SUPPORT / "host-dependencies.json"
    scope_path = SUPPORT / "source-scope.json"
    scope = source_scope()
    if args.write_index:
        path.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")
        scope_path.write_text(json.dumps(scope, indent=2) + "\n", encoding="utf-8")
    reviewed_scope = set(json.loads(scope_path.read_text(encoding="utf-8")))
    changed_scope = reviewed_scope.symmetric_difference(scope)
    if changed_scope:
        raise SystemExit(
            "Runtime files entered or left the reviewed source scope:\n"
            + "\n".join(sorted(changed_scope))
        )
    previous: dict[str, list[str]] = json.loads(path.read_text(encoding="utf-8"))
    if args.output:
        policy = load_test_policy(ROOT / "TEST_POLICY.toml")
        owners = {p: [a.name for a in policy.areas if a.owns(p)] for p in assumptions}
        with (SUPPORT / "host-contracts.toml").open("rb") as source:
            contracts = tomllib.load(source)["contract"]
        local_tests = assertion_sources(set(assumptions))
        from tools.compatibility.characterization import write_characterizations

        write_characterizations(
            ROOT, args.output.with_name("local-characterizations.json"), local_tests
        )
        args.output.write_text(
            json.dumps(
                [
                    {
                        **asdict(e),
                        "area": owners[e.path],
                        "api_status": api_status(e),
                        "direct_local_test_imports": local_tests.get(e.path, []),
                        "contracts": [
                            c["id"] for c in contracts if e.path in c["owners"]
                        ],
                    }
                    for e in entries
                ],
                indent=2,
            ),
            encoding="utf-8",
        )
    drift = sorted(
        p for p in previous.keys() | index.keys() if previous.get(p) != index.get(p)
    )
    if drift:
        raise SystemExit(
            "Changed host dependencies require review:\n" + "\n".join(drift)
        )
    print(f"{len(entries)} host touch points indexed across {len(index)} source files")


if __name__ == "__main__":
    main()
