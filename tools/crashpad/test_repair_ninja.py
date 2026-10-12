# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Observe exact path correction and rejection without compiler or native effects."""
import unittest
from repair_ninja import repair_text

SOURCE = "E:/devprojects/SugarSubstitute-Rust-l5-crash/build/crashpad-source/crashpad"
OUTPUT = "D:/SugarSubstitute-Rust/artifacts/l5/crash-01/native-build/out-msvc-x64"
BAD = "../" * 7 + SOURCE


class RepairTests(unittest.TestCase):
    def repair(self, text):
        return repair_text(text, SOURCE, OUTPUT)

    def test_source_edges_and_escaped_include_paths_are_exact(self):
        text = "build obj/a.obj: cxx " + BAD + "/client/a.cc\ninclude_dirs = -I" + BAD.replace("E:", "E$:") + " -Igen\n"
        self.assertEqual(self.repair(text), "build obj/a.obj: cxx " + SOURCE.replace("E:", "E$:") + "/client/a.cc\ninclude_dirs = -I" + SOURCE.replace("E:", "E$:") + " -Igen\n")

    def test_known_buildflag_script_preserves_other_commands_and_variables(self):
        script = "/third_party/mini_chromium/mini_chromium/build/write_buildflag_header.py"
        text = '  command = C$:/Python/python.exe ' + BAD + script + ' --out ${out} --argument="a$ b"\n  command = cl.exe /Fd"Program$ Files/a.pdb" ${in}\n'
        expected = text.replace(BAD + script, SOURCE.replace("E:", "E$:") + script)
        self.assertEqual(self.repair(text), expected)

    def test_unrelated_content_is_byte_preserved(self):
        text = '# Windows paths C$:/SDK and ${variable}\n  command = cmd /c copy ${in} ${out}\r\n'
        self.assertEqual(self.repair(text), text)

    def test_foreign_drive_and_source_are_rejected(self):
        for bad in (BAD.replace("E:", "C:"), BAD.replace("crashpad-source", "other-source"), "../" + BAD):
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                self.repair("build a: cxx " + bad + "/a.cc\n")

    def test_traversal_dollar_quote_backslash_are_rejected(self):
        for tail in ("/../private.cc", "/./a.cc", "/a$.cc", '/"a.cc', r"/a\b.cc", "/a:stream"):
            with self.subTest(tail=tail), self.assertRaises(ValueError):
                self.repair("build a: cxx " + BAD + tail + "\n")

    def test_unknown_positions_and_command_sources_are_rejected(self):
        for line in ("  command = cl.exe " + BAD + "/a.cc", "note = " + BAD,
                     "  command = python.exe " + BAD + "/unknown/write_buildflag_header.py --out x"):
            with self.subTest(line=line), self.assertRaises(ValueError):
                self.repair(line + "\n")

    def test_wrong_drive_roots_are_rejected(self):
        for source, output in ((SOURCE.replace("E:", "C:"), OUTPUT), (SOURCE, OUTPUT.replace("D:", "E:"))):
            with self.assertRaises(ValueError):
                repair_text("", source, output)

    def test_exact_wer_definition_is_repaired_without_other_flag_changes(self):
        text = "  ldflags = /DEBUG /OPT$:REF /DEF:" + BAD + "/handler/win/wer/crashpad_wer.def\n"
        self.assertEqual(self.repair(text), text.replace(BAD, SOURCE.replace("E:", "E$:")))
        for text in ("  ldflags = /LIBPATH:" + BAD + "/handler/win/wer/crashpad_wer.def", "  ldflags = /DEF:" + BAD + "/other.def"):
            with self.assertRaises(ValueError):
                self.repair(text)


if __name__ == "__main__":
    unittest.main()
