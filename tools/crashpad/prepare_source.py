# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Apply exact reviewed derivatives to the immutable local Crashpad source input."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import tarfile

PINNED = {
    "client/crashpad_client.h": "7c7b56bb58f7e1adc102be6a3b257d0c8535b3da737a02fb1fa07140a33e1658",
    "client/crashpad_client_win.cc": "034724f0aa6295903908564eeb9326ca8d05a33b924bd27b003790e197beb4bb",
    "client/BUILD.gn": "da905f5165e272c5ba5b976d0c8de8464a885eb9a69bdbbc9670b4674ab60896",
    "third_party/mini_chromium/mini_chromium/build/win_helper.py": "efab24ae793a93d65bafb0709d6524420281f01acbdce96c9bff156dd6528bc1",
}


def replace(text, old, new):
    if text.count(old) != 1:
        raise ValueError("Reviewed patch anchor is not unique")
    return text.replace(old, new)


def transform_owned_launch(text):
    """Bind registration before child creation and retain every partial output."""
    text = replace(text, "  PROCESS_INFORMATION process_info;", "  if (command_line.size() >= 32767 || (data->owner && is_embedded_in_dll)) {\n    if (data->owner) data->owner->error = ERROR_INVALID_PARAMETER;\n    SetLastError(ERROR_INVALID_PARAMETER); return false;\n  }\n  if (data->owner && !sugarsubstitute::PrepareInheritedRegistration(data->ipc_pipe, data->owner)) return false;\n  if (data->owner && !sugarsubstitute::CheckDeadline(data->owner)) return false;\n  PROCESS_INFORMATION process_info = {};")
    text = replace(text, "  if (!rv) {\n    PLOG(ERROR) << \"CreateProcess\";", """  const DWORD create_error = rv ? ERROR_SUCCESS : GetLastError();
  if (data->owner) {
    // Adopt partial outputs before examining success or any later fallible call.
    data->owner->process = process_info.hProcess;
    data->owner->thread = process_info.hThread;
  }
  if (!rv) {
    if (data->owner && !data->owner->error) data->owner->error = create_error;
    SetLastError(create_error);
    PLOG(ERROR) << "CreateProcess";""")
    return replace(text, "  rv = CloseHandle(process_info.hThread);", "  if (data->owner) {\n    // The connected endpoint and original server stay owned through confirmation.\n    if (!sugarsubstitute::ConfirmHandler(data->owner)) return false;\n    scoped_startup_state_caller.SetSuccessful();\n    return true;\n  }\n\n  rv = CloseHandle(process_info.hThread);")


def transform(name, text):
    if name == "client/crashpad_client.h":
        text = replace(text, "namespace crashpad {", "namespace sugarsubstitute { struct OwnedStart; }\n\nnamespace crashpad {")
        text = replace(text, "  bool SetHandlerIPCPipe(const std::wstring& ipc_pipe);", """  bool SetHandlerIPCPipe(const std::wstring& ipc_pipe);

  //! Starts once synchronously, retaining every native partial output in owner.
  //! owner lives until all OVERLAPPED completion/checked closure is observed.
  bool StartHandlerOwned(const base::FilePath& handler,
                         const base::FilePath& database,
                         const base::FilePath& metrics_dir,
                         const std::map<std::string, std::string>& annotations,
                         sugarsubstitute::OwnedStart* owner);""")
        return replace(text, "  std::wstring ipc_pipe_;", "  sugarsubstitute::OwnedStart* owned_start_ = nullptr;\n  std::wstring ipc_pipe_;")
    if name == "client/BUILD.gn":
        return replace(text, '      "crashpad_client_win.cc",', '      "crashpad_client_win.cc",\n      "../sugarsubstitute/owned_registration.cc",\n      "../sugarsubstitute/owned_start.h",')
    if name.endswith("win_helper.py"):
        text = replace(text, "    archs = ('x86', 'amd64', 'arm64')", "    archs = ('amd64',) if os.environ.get('SUGAR_CRASHPAD_VS') else ('x86', 'amd64', 'arm64')")
        text = replace(text, "        args.extend((script_arch_name, '&&', 'set'))", """        if os.environ.get('SUGAR_CRASHPAD_VS'):
            args.extend((script_arch_name, '10.0.26100.0', '-vcvars_ver=14.43', '&&', 'set'))
        else:
            args.extend((script_arch_name, '&&', 'set'))""")
        text = replace(text, "        install_dir, script_path = (explicit() or env() or autodetect() or\n                                    fail())", """        reviewed = os.environ.get('SUGAR_CRASHPAD_VS')
        if reviewed:
            if os.path.normcase(os.path.abspath(toolchain_path)) != os.path.normcase(os.path.abspath(reviewed)):
                raise ValueError('Explicit GN toolchain does not match reviewed VS instance')
            install_dir, script_path = reviewed, os.path.join('VC', 'Auxiliary', 'Build', 'vcvarsall.bat')
        else:
            install_dir, script_path = (explicit() or env() or autodetect() or fail())""")
        return replace(text, "        x86_file, x64_file, arm64_file = _GenerateEnvironmentFiles(\n            install_dir, outdir, script_path)", """        files = _GenerateEnvironmentFiles(install_dir, outdir, script_path)
        if reviewed:
            # Empty entries deliberately forbid selecting an unprepared architecture.
            x86_file, x64_file, arm64_file = '', files[0], ''
        else:
            x86_file, x64_file, arm64_file = files""")
    text = replace(text, '#include "client/crashpad_client.h"', '#include "client/crashpad_client.h"\n#include "sugarsubstitute/owned_start.h"')
    text = replace(text, "void CreatePipe(std::wstring* pipe_name, ScopedFileHANDLE* pipe_instance)", "bool CreatePipe(std::wstring* pipe_name, ScopedFileHANDLE* pipe_instance)")
    text = replace(text, '  PCHECK(pipe_instance->is_valid()) << "CreateNamedPipe";', '  return pipe_instance->is_valid();')
    text = replace(text, "  base::FilePath handler;", "  sugarsubstitute::OwnedStart* owner = nullptr;\n  base::FilePath handler;")
    text = replace(text, '    PLOG(ERROR) << "OpenProcess";', '    if (data->owner && !data->owner->error) data->owner->error = GetLastError();\n    PLOG(ERROR) << "OpenProcess";')
    text = replace(text, "      this_process.get(),", "      data->owner ? data->owner->bootstrap_process : this_process.get(),")
    text = replace(text, "  InitialClientData initial_client_data(", "  if (data->owner && this_process.is_valid()) data->owner->bootstrap_process = this_process.release();\n  InitialClientData initial_client_data(")
    text = replace(text, "  startup_info.StartupInfo.dwFlags =\n      STARTF_USESTDHANDLES | STARTF_FORCEOFFFEEDBACK;", "  startup_info.StartupInfo.dwFlags = STARTF_FORCEOFFFEEDBACK |\n      (data->owner ? 0 : STARTF_USESTDHANDLES);")
    if text.count("data->ipc_pipe_handle.get()") != 2:
        raise ValueError("Reviewed bootstrap pipe anchors changed")
    text = text.replace("data->ipc_pipe_handle.get()", "(data->owner ? data->owner->initial_pipe : data->ipc_pipe_handle.get())")
    text = replace(text, "    handle_list.push_back(this_process.get());", "    handle_list.push_back(data->owner ? data->owner->bootstrap_process : this_process.get());")
    text = replace(text, "  startup_info.StartupInfo.hStdInput = GetStdHandle(STD_INPUT_HANDLE);\n  startup_info.StartupInfo.hStdOutput = GetStdHandle(STD_OUTPUT_HANDLE);\n  startup_info.StartupInfo.hStdError = GetStdHandle(STD_ERROR_HANDLE);", """  // The owned path never inherits ambient stdio/bootstrap handles.
  if (!data->owner) {
    startup_info.StartupInfo.hStdInput = GetStdHandle(STD_INPUT_HANDLE);
    startup_info.StartupInfo.hStdOutput = GetStdHandle(STD_OUTPUT_HANDLE);
    startup_info.StartupInfo.hStdError = GetStdHandle(STD_ERROR_HANDLE);
  }""")
    text = replace(text, "    creation_flags = 0;", "    if (data->owner) { SetLastError(ERROR_NOT_SUPPORTED); return false; }\n    creation_flags = 0;")
    text = transform_owned_launch(text)
    text = replace(text, "  CreatePipe(&ipc_pipe_, &ipc_pipe_handle);", "  if (!CreatePipe(&ipc_pipe_, &ipc_pipe_handle)) {\n    if (owned_start_) owned_start_->error = GetLastError();\n    SetHandlerStartupState(StartupState::kFailed);\n    return false;\n  }\n  if (owned_start_) owned_start_->initial_pipe = ipc_pipe_handle.release();")
    for name in ("g_signal_exception", "g_wer_registration.dump_without_crashing", "g_wer_registration.dump_completed"):
        old = name + " =\n      CreateEvent(&security_attributes, false /* auto reset */, false, nullptr);"
        text = replace(text, old, old + "\n  if (!" + name + " && owned_start_ && !owned_start_->error) owned_start_->error = GetLastError();")
    for operation in ("InitializeProcThreadAttributeList (size)", "InitializeProcThreadAttributeList", "UpdateProcThreadAttribute"):
        old = '      PLOG(ERROR) << "' + operation + '";'
        text = replace(text, old, '      if (data->owner && !data->owner->error) data->owner->error = GetLastError();\n' + old)
    text = replace(text, "  CommonInProcessInitialization();\n\n  RegisterHandlers();", "  if (!g_signal_exception || !g_wer_registration.dump_without_crashing ||\n      !g_wer_registration.dump_completed) {\n    SetHandlerStartupState(StartupState::kFailed);\n    return false;\n  }\n  CommonInProcessInitialization();\n\n  RegisterHandlers();")
    text = replace(text, "  if (asynchronous_start) {", "  data->owner = owned_start_;\n  if (asynchronous_start) {")
    return replace(text, "void CrashpadClient::RegisterHandlers() {", """bool CrashpadClient::StartHandlerOwned(
    const base::FilePath& handler, const base::FilePath& database,
    const base::FilePath& metrics_dir,
    const std::map<std::string, std::string>& annotations,
    sugarsubstitute::OwnedStart* owner) {
  if (!owner || owned_start_ || !ipc_pipe_.empty()) {
    SetLastError(ERROR_ALREADY_INITIALIZED);
    return false;
  }
  owned_start_ = owner;
  return StartHandler(handler, database, metrics_dir, "", annotations,
                      {"--no-periodic-tasks"}, false, false);
}

void CrashpadClient::RegisterHandlers() {""")


def prepare(source, authored, receipt, downloads):
    from verify_source import authored_inputs
    authored_inputs(authored)
    edits = []
    # Validate the complete immutable set before changing any file.
    inputs = {}
    previous = {item["file"]: item.get("patched_sha256") for item in json.loads(receipt.read_text())} if receipt.exists() else {}
    archives = {
        "crashpad": ("96dfd1ecdf44095f1feaf893ce189d267707d11b5866f8f3474ee34188bdc2b8", "crashpad-60dd943f48d77dc8d05dabc04badbd8561d0b8c4/"),
        "mini_chromium": ("e35ed2d041232fde0bbfbc1517431c3902890526c696d64ee062dc370baf2b9d", "mini_chromium-e5169551c51f3a52eee36b3b03f219cefe380237/"),
    }
    for package, (expected, _) in archives.items():
        if hashlib.sha256((downloads / (package + ".tar.gz")).read_bytes()).hexdigest() != expected:
            raise ValueError("Immutable archive mismatch")
    for name, expected in PINNED.items():
        current = hashlib.sha256((source / name).read_bytes()).hexdigest()
        if current not in (expected, previous.get(name)):
            raise ValueError(f"Unexpected cache modification: {name}")
        mini_prefix = "third_party/mini_chromium/mini_chromium/"
        package = "mini_chromium" if name.startswith(mini_prefix) else "crashpad"
        relative = name[len(mini_prefix):] if package == "mini_chromium" else name
        with tarfile.open(downloads / (package + ".tar.gz")) as archive:
            stream = archive.extractfile(archives[package][1] + relative)
            if stream is None:
                raise ValueError("Pinned file missing from immutable archive")
            raw = stream.read()
        if hashlib.sha256(raw).hexdigest() != expected:
            raise ValueError(f"Pinned source mismatch: {name}")
        inputs[name] = raw.decode("utf-8")
    transformed = {name: transform(name, original).encode("utf-8") for name, original in inputs.items()}
    for name, raw in transformed.items():
        (source / name).write_bytes(raw)
        edits.append({"file": name, "original_sha256": PINNED[name], "patched_sha256": hashlib.sha256(raw).hexdigest()})
    destination = source / "sugarsubstitute"
    destination.mkdir(exist_ok=True)
    for path in sorted(authored.iterdir()):
        shutil.copyfile(path, destination / path.name)
        edits.append({"file": "sugarsubstitute/" + path.name, "authored_sha256": hashlib.sha256(path.read_bytes()).hexdigest()})
    receipt.write_text(json.dumps(edits, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--authored", required=True, type=Path)
    parser.add_argument("--receipt", required=True, type=Path)
    parser.add_argument("--downloads", required=True, type=Path)
    args = parser.parse_args()
    prepare(args.source, args.authored, args.receipt, args.downloads)
