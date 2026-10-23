// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Consume SugarCubes' versioned public model-root owner without reproducing its registry or persistence. */
export const configureModelRoot = `
import importlib.util, pathlib, sys
extension, comfy, models = map(pathlib.Path, sys.argv[1:4])
spec = importlib.util.spec_from_file_location("substitute_setup_model_root", extension / "load_model_roots.py")
if spec is None or spec.loader is None:
    raise RuntimeError("SugarCubes model-root loader is unavailable")
loader = importlib.util.module_from_spec(spec)
spec.loader.exec_module(loader)
api = loader.load_model_root_api()
if getattr(api, "MODEL_ROOT_API_VERSION", None) != 1:
    raise RuntimeError("SugarCubes model-root API version is unsupported")
store = api.ModelRootStore(comfy)
configuration = api.ModelRootConfiguration(store, lambda: store.default_root)
configuration.configure("custom", str(models))
if store.load() != models.resolve():
    raise RuntimeError("SugarCubes did not preserve the selected model root")
`
