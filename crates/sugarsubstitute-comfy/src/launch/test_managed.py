# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Qualify the real managed interpreter shutdown against an idle asyncio backend."""
from pathlib import Path
import os
import subprocess
import sys
import tempfile
import queue
import threading
import unittest
import json


def bootstrap(owned):
    """Assemble the same immutable fragments used by the sole Rust converter."""
    root = Path(__file__).parent
    common = (root / "managed_start.py").read_text(encoding="utf-8")
    finish = (root / "managed_finish.py").read_text(encoding="utf-8")
    if not owned:
        return common + finish
    prefix = (root / "managed_owned.py").read_text(encoding="utf-8")
    return prefix + common + "    validate_owned_model_selection()\n" + finish


class ManagedShutdownTests(unittest.TestCase):
    def test_eof_interrupts_idle_server_and_runs_finally(self):
        source = bootstrap(False)
        with tempfile.TemporaryDirectory(prefix="sugar-managed-shutdown-") as directory:
            root = Path(directory)
            (root / "comfy").mkdir()
            (root / "comfy" / "__init__.py").write_text("", encoding="utf-8")
            (root / "comfy" / "options.py").write_text(
                "enabled = False\n"
                "def enable_args_parsing():\n"
                "    global enabled\n"
                "    enabled = True\n", encoding="utf-8",
            )
            (root / "folder_paths.py").write_text(
                "from comfy import options\n"
                "assert options.enabled\n"
                "paths = {'custom_nodes': ['user/comfy-data/custom_nodes']}\n"
                "def add_model_folder_path(kind, path):\n"
                "    if path not in paths[kind]: paths[kind].append(path)\n", encoding="utf-8",
            )
            script = Path(directory) / "main.py"
            script.write_text(
                "import asyncio, os, sys, folder_paths\n"
                "assert folder_paths.paths['custom_nodes'] == [\n"
                "    'user/comfy-data/custom_nodes',\n"
                "    os.path.join(os.path.dirname(__file__), 'custom_nodes')]\n"
                "os.fstat(sys.stdin.fileno())\n"
                "sys.stdin.isatty()\n"
                "assert os.read(sys.stdin.fileno(), 1) == b''\n"
                "async def serve():\n"
                "    print('ready:' + sys.argv[1], flush=True)\n"
                "    await asyncio.Event().wait()\n"
                "loop = asyncio.new_event_loop()\n"
                "asyncio.set_event_loop(loop)\n"
                "try:\n"
                "    loop.run_until_complete(serve())\n"
                "finally:\n"
                "    print('cleaned', flush=True)\n",
                encoding="utf-8",
            )
            child = subprocess.Popen(
                [sys.executable, "-u", "-c", source, str(script), "café"],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                text=True, encoding="utf-8",
                env={**os.environ, "PYTHONIOENCODING": "utf-8"},
                creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
            )
            try:
                ready = queue.Queue()
                reader = threading.Thread(target=lambda: ready.put(child.stdout.readline()), daemon=True)
                reader.start()
                self.assertEqual(ready.get(timeout=5).strip(), "ready:café")
                reader.join(timeout=5)
                self.assertIsNone(child.poll())
                child.stdin.close()
                child.stdin = None
                output, error = child.communicate(timeout=10)
                self.assertEqual(child.returncode, 0, error)
                self.assertIn("cleaned", output)
            finally:
                if child.poll() is None:
                    child.kill()
                child.communicate(timeout=10)


class OwnedModelSelectionTests(unittest.TestCase):
    def invoke(self, mode, owned=True):
        with tempfile.TemporaryDirectory(prefix="sugar-owned-model-selection-") as directory:
            root = Path(directory)
            storage = root / "user" / "comfy-data"
            cubes = root / "selected SugarCubes café"
            cubes.mkdir()
            (root / "comfy").mkdir()
            (root / "comfy" / "__init__.py").write_text("", encoding="utf-8")
            (root / "comfy" / "options.py").write_text(
                "def enable_args_parsing(): pass\n", encoding="utf-8"
            )
            (root / "folder_paths.py").write_text(
                f"base_path = {str(storage)!r}\n"
                "def add_model_folder_path(kind, path): pass\n", encoding="utf-8"
            )
            trace = cubes / "read-trace.json"
            method = "pass\n" if mode == "missing" else (
                "def read_preserved_selection(self):\n"
                f"        open({str(trace)!r}, 'w', encoding='utf-8').write(json.dumps(str(self.root)))\n"
                + ("        raise ValueError('selected original refused') from OSError('original owner cause')\n"
                   if mode == "refused" else "        return None\n")
            )
            (cubes / "load_model_roots.py").write_text(
                "import json, types\n"
                "class Store:\n"
                "    def __init__(self, root): self.root = root\n"
                "    " + method +
                "def load_model_root_api():\n"
                "    return types.SimpleNamespace(MODEL_ROOT_API_VERSION=1, ModelRootStore=Store)\n",
                encoding="utf-8",
            )
            main = root / "main.py"
            main.write_text(
                "import sys\nprint('main:' + sys.argv[1], flush=True)\n", encoding="utf-8"
            )
            arguments = [str(cubes), str(main), "日本語"] if owned else [str(main), "日本語"]
            child = subprocess.Popen(
                [sys.executable, "-B", "-u", "-c", bootstrap(owned), *arguments],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                text=True, encoding="utf-8",
                env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1", "PYTHONIOENCODING": "utf-8"},
                creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
            )
            try:
                child.wait(timeout=10)
                output, error = child.communicate(timeout=10)
                read = json.loads(trace.read_text(encoding="utf-8")) if trace.exists() else None
                self.assertFalse(storage.exists())
                return child.returncode, output, error, read, str(storage)
            finally:
                if child.poll() is None:
                    child.kill()
                child.communicate(timeout=10)

    def test_owned_reads_public_selection_at_actual_comfy_base_before_main(self):
        code, output, error, read, storage = self.invoke("valid")
        self.assertEqual(code, 0, error)
        self.assertEqual(output.strip(), "main:日本語")
        self.assertEqual(read, storage)

    def test_owned_missing_public_reader_refuses_before_main_without_storage_writes(self):
        code, output, error, read, _ = self.invoke("missing")
        self.assertNotEqual(code, 0)
        self.assertEqual(output, "")
        self.assertIsNone(read)
        self.assertIn("preserved model-root reader is unavailable", error)

    def test_owned_refusal_retains_public_cause_and_never_executes_main(self):
        code, output, error, read, storage = self.invoke("refused")
        self.assertNotEqual(code, 0)
        self.assertEqual(output, "")
        self.assertEqual(read, storage)
        self.assertIn("selected original refused", error)
        self.assertIn("original owner cause", error)

    def test_external_keeps_normal_bootstrap_even_when_new_public_reader_is_missing(self):
        code, output, error, read, _ = self.invoke("missing", owned=False)
        self.assertEqual(code, 0, error)
        self.assertEqual(output.strip(), "main:日本語")
        self.assertIsNone(read)


if __name__ == "__main__":
    unittest.main()
