// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Inspect package requirements with the selected interpreter before and after integration. */
export const inspectProgram = `
import json, pathlib, sys, tomllib, importlib.metadata as metadata
from packaging.requirements import Requirement
from packaging.specifiers import SpecifierSet
project_path = pathlib.Path(sys.argv[1])
if project_path.stat().st_size > 65536:
    raise ValueError("SugarCubes metadata exceeds its bound")
project = tomllib.loads(project_path.read_text(encoding="utf-8"))["project"]
if project.get("name") != "SugarCubes":
    raise ValueError("SugarCubes package identity is invalid")
if not SpecifierSet(project.get("requires-python", "")).contains(".".join(map(str, sys.version_info[:3]))):
    raise ValueError("SugarCubes requires a different Python version")
requirements = project.get("dependencies", [])
if not isinstance(requirements, list) or len(requirements) > 128:
    raise ValueError("SugarCubes dependency inventory exceeds its bound")
missing = []
for text in requirements:
    if not isinstance(text, str) or len(text) > 1024:
        raise ValueError("SugarCubes dependency is invalid")
    requirement = Requirement(text)
    if requirement.url:
        raise ValueError("SugarCubes dependencies must use package registry requirements")
    if requirement.marker and not requirement.marker.evaluate():
        continue
    try:
        satisfied = requirement.specifier.contains(metadata.version(requirement.name))
    except metadata.PackageNotFoundError:
        satisfied = False
    if not satisfied:
        missing.append(str(requirement))
print(json.dumps(missing))
`
