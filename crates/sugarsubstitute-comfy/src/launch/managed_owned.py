# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
# This prefix is selected only by the parent-authenticated owned-data launch plan.
import sys as _owned_sys
_owned_cubes = _owned_sys.argv.pop(1)


def validate_owned_model_selection():
    """Ask the public owner to validate copied intent at Comfy's actual parsed base."""
    import importlib.util
    from pathlib import Path
    import folder_paths

    spec = importlib.util.spec_from_file_location(
        "substitute_owned_model_root", Path(_owned_cubes) / "load_model_roots.py"
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("SugarCubes model-root loader is unavailable")
    loader = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loader)
    api = loader.load_model_root_api()
    if getattr(api, "MODEL_ROOT_API_VERSION", None) != 1:
        raise RuntimeError("SugarCubes model-root API version is unsupported")
    store = api.ModelRootStore(Path(folder_paths.base_path))
    read_preserved = getattr(store, "read_preserved_selection", None)
    if not callable(read_preserved):
        raise RuntimeError("SugarCubes preserved model-root reader is unavailable")
    read_preserved()


