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


class ManagedShutdownTests(unittest.TestCase):
    def test_eof_interrupts_idle_server_and_runs_finally(self):
        source = Path(__file__).with_name("managed.py").read_text(encoding="utf-8")
        with tempfile.TemporaryDirectory(prefix="sugar-managed-shutdown-") as directory:
            script = Path(directory) / "main.py"
            script.write_text(
                "import asyncio, os, sys\n"
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


if __name__ == "__main__":
    unittest.main()
