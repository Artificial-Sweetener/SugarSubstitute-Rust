# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Verify launch capability ordering and fail-closed derivative admission."""
import unittest
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
        deadline = result.index("CheckDeadline(data->owner)")
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


if __name__ == "__main__":
    unittest.main()
