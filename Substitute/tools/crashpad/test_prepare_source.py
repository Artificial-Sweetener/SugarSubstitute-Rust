# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Verify launch capability ordering and fail-closed derivative admission."""
import unittest
import importlib.util
from pathlib import Path
from prepare_source import transform_owned_launch


LAUNCH = '''  PROCESS_INFORMATION process_info;
  rv = CreateProcess(application, command, nullptr, nullptr, true,
      creation_flags, nullptr, nullptr, &startup_info.StartupInfo, &process_info);
  if (!rv) {
    PLOG(ERROR) << "CreateProcess";
    return false;
  }
  rv = CloseHandle(process_info.hThread);
'''


class OwnedLaunchTests(unittest.TestCase):
    def test_endpoint_authentication_and_original_deadline_precede_process_creation(self):
        result = transform_owned_launch(LAUNCH)
        prepare = result.index("PrepareInheritedRegistration(data->ipc_pipe, data->owner)")
        deadline = result.index("data->owner->connection.CheckDeadline()")
        create = result.index("rv = CreateProcess(")
        self.assertLess(prepare, deadline)
        self.assertLess(deadline, create)
        self.assertIn("PrepareInheritedRegistration(data->ipc_pipe, data->owner)) return false;", result)
        self.assertIn("PROCESS_INFORMATION process_info = {};", result)

    def test_both_partial_process_outputs_are_adopted_before_failure_status(self):
        result = transform_owned_launch(LAUNCH)
        cause = result.index("const DWORD create_error")
        process = result.index("data->owner->process = process_info.hProcess;")
        thread = result.index("data->owner->thread = process_info.hThread;")
        failure = result.index("if (!rv)")
        self.assertLess(cause, process)
        self.assertLess(process, thread)
        self.assertLess(thread, failure)
        self.assertIn("SetLastError(create_error);", result)

    def test_confirmation_uses_retained_capability_without_closing_inherited_instance(self):
        result = transform_owned_launch(LAUNCH)
        confirmation = result.index("ConfirmHandler(data->owner)")
        success = result.index("scoped_startup_state_caller.SetSuccessful()")
        normal_close = result.index("CloseHandle(process_info.hThread)")
        self.assertLess(confirmation, success)
        self.assertLess(success, normal_close)
        self.assertNotIn("CloseHandle(data->owner->initial_pipe)", result)
        self.assertNotIn("ConfirmHandler(data->ipc_pipe", result)

    def test_each_missing_or_duplicate_anchor_rejects_the_derivative(self):
        for anchor in ("  PROCESS_INFORMATION process_info;",
                       '  if (!rv) {\n    PLOG(ERROR) << "CreateProcess";',
                       "  rv = CloseHandle(process_info.hThread);"):
            for source in (LAUNCH.replace(anchor, "changed"), LAUNCH + anchor):
                with self.subTest(anchor=anchor, source=source), self.assertRaises(ValueError):
                    transform_owned_launch(source)


class RuntimeExtractionTests(unittest.TestCase):
    """Reject source evidence ambiguity before generating compiled native tests."""
    def setUp(self):
        path = Path(__file__).resolve().parents[2] / "native/crashpad/capture_runtime_test_source.py"
        spec = importlib.util.spec_from_file_location("runtime_test_owner", path)
        self.owner = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.owner)
        self.client = ("bool CommonInProcessInitialization(owner) { /* café */ }\n"
                       "}  // namespace\n\nCrashpadClient::CrashpadClient() {}\n"
                       "bool CrashpadClient::InitializeCapturedAttach(owner) {}\n"
                       "bool CrashpadClient::SetHandlerIPCPipe(pipe) {}\n")
        self.helper = "namespace { bool actual_helper() {} }  // namespace crashpad\n"

    def test_exact_utf8_owner_bodies_are_retained_without_text_conversion(self):
        result = self.owner.extract(self.client.encode("utf-8"), self.helper.encode("utf-8"))
        self.assertEqual(result["capture_runtime_common.inc"],
                         "bool CommonInProcessInitialization(owner) { /* café */ }".encode("utf-8"))
        self.assertEqual(result["capture_runtime_methods.inc"],
                         b"bool CrashpadClient::InitializeCapturedAttach(owner) {}\n")
        self.assertEqual(result["capture_runtime_helper.inc"],
                         b"namespace { bool actual_helper() {} ")

    def test_missing_or_duplicate_source_boundaries_reject(self):
        for anchor in ("bool CommonInProcessInitialization(",
                       "\n}  // namespace\n\nCrashpadClient::CrashpadClient()",
                       "bool CrashpadClient::InitializeCapturedAttach(",
                       "bool CrashpadClient::SetHandlerIPCPipe("):
            for source in (self.client.replace(anchor, "changed"), self.client + anchor):
                with self.subTest(anchor=anchor), self.assertRaises(ValueError):
                    self.owner.extract(source.encode("utf-8"), self.helper.encode("utf-8"))
        for anchor in ("namespace {", "}  // namespace crashpad"):
            for helper in (self.helper.replace(anchor, "changed"), self.helper + anchor):
                with self.subTest(anchor=anchor), self.assertRaises(ValueError):
                    self.owner.extract(self.client.encode("utf-8"), helper.encode("utf-8"))

    def test_reversed_owner_boundaries_reject(self):
        with self.assertRaises(ValueError):
            self.owner.between("end\nstart", "start", "end")

    def test_cp1252_source_is_rejected_instead_of_reencoded(self):
        with self.assertRaises(UnicodeDecodeError):
            self.owner.extract(self.client.encode("cp1252"), self.helper.encode("utf-8"))


if __name__ == "__main__":
    unittest.main()
