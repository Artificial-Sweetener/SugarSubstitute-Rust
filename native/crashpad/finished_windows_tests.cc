// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "finished_report.h"
#include <sddl.h>
#include <cstdio>
#include <string>

namespace sf = sugarsubstitute::finished;
namespace {
// Explicit test-owned native outputs are retired even when a check fails. The
// production owner is exercised separately; this qualifies its actual OS port.
struct Fixture {
  HANDLE token = nullptr, directory = nullptr, file = nullptr, inspection = nullptr;
  PSECURITY_DESCRIPTOR descriptor = nullptr;
  LPWSTR sid_text = nullptr;
  std::wstring root;
  bool made = false;
  bool Close() {
    auto& api = sf::WindowsFiles(); bool ok = true;
    for (auto slot : {&inspection, &file, &directory, &token}) {
      if (*slot) { if (api.Close(*slot)) ok = false; else *slot = nullptr; }
    }
    if (made && !directory && !file && !inspection) {
      const auto path = root + L"\\native.stage";
      if (!DeleteFileW(path.c_str()) && GetLastError() != ERROR_FILE_NOT_FOUND) ok = false;
      if (!RemoveDirectoryW(root.c_str())) ok = false; else made = false;
    }
    if (descriptor) { if (LocalFree(descriptor)) ok = false; else descriptor = nullptr; }
    if (sid_text) { if (LocalFree(sid_text)) ok = false; else sid_text = nullptr; }
    return ok;
  }
  bool Start(const wchar_t* input) {
    root = input;
    constexpr wchar_t prefix[] = L"D:\\SugarSubstitute-Rust\\artifacts\\k1\\finished-native01\\";
    if (!root.starts_with(prefix) || root.find(L"..") != std::wstring::npos ||
        GetFileAttributesW(root.c_str()) != INVALID_FILE_ATTRIBUTES) return false;
    if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token)) return false;
    std::array<uint32_t, 256> buffer{}; DWORD needed = 0;
    if (!GetTokenInformation(token, TokenUser, buffer.data(), static_cast<DWORD>(sizeof(buffer)), &needed) || needed > sizeof(buffer)) return false;
    const auto user = reinterpret_cast<TOKEN_USER*>(buffer.data());
    if (!ConvertSidToStringSidW(user->User.Sid, &sid_text)) return false;
    const std::wstring text = L"O:" + std::wstring(sid_text) + L"D:P(A;OICI;FA;;;" + sid_text + L")(A;OICI;FA;;;SY)S:(ML;;NW;;;ME)";
    if (!ConvertStringSecurityDescriptorToSecurityDescriptorW(text.c_str(), SDDL_REVISION_1, &descriptor, nullptr)) return false;
    SECURITY_ATTRIBUTES attributes{sizeof(attributes), descriptor, FALSE};
    if (!CreateDirectoryW(root.c_str(), &attributes)) return false;
    made = true;
    directory = CreateFileW(root.c_str(), FILE_LIST_DIRECTORY | READ_CONTROL | SYNCHRONIZE,
        FILE_SHARE_READ | FILE_SHARE_WRITE, nullptr, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT, nullptr);
    if (directory == INVALID_HANDLE_VALUE) { directory = nullptr; return false; }
    return true;
  }
};
class FixtureControl final : public sf::ParseControl {public: sf::Checkpoint Check() noexcept override {return {};}};
class StoppingControl final : public sf::ParseControl {
 public:
  uint32_t calls=0, stop=1;
  sf::Checkpoint Check() noexcept override {
    return ++calls>=stop?sf::Checkpoint{sf::Interruption::Cancelled,ERROR_OPERATION_ABORTED}:sf::Checkpoint{};
  }
};
SsFinishedControlStatusV1 __cdecl Current(uintptr_t,uint32_t* cause) noexcept { *cause=0;return FinishedControlCurrent; }
SsFinishedControlStatusV1 __cdecl Cancelled(uintptr_t,uint32_t* cause) noexcept { *cause=ERROR_OPERATION_ABORTED;return FinishedControlCancelled; }
bool KnownNoOwner(const SsFinishedResultV1& result, DWORD cause, SsFinishedCauseOriginV1 origin) {
  return result.abi_version==1&&result.retired==1&&!result.lease&&!result.live_handles&&!result.live_locks&&
      !result.facts_valid&&!result.actual_read_bytes&&!result.actual_reclaimed_bytes&&result.primary_error==cause&&result.primary_origin==origin;
}
bool NoOwnerCreation(Fixture& fixture) {
  LARGE_INTEGER now{},frequency{};
  if(!QueryPerformanceCounter(&now)||!QueryPerformanceFrequency(&frequency))return false;
  SsFinishedOpenInputV1 input{};input.abi_version=1;
  input.borrowed_database=reinterpret_cast<uintptr_t>(fixture.directory);input.borrowed_metrics=input.borrowed_database;
  input.subject.tag=Desktop;input.subject.epoch[0]=1;input.subject.instance=1;input.subject.generation=1;
  input.build_length=1;input.build[0]='a';input.original_operation={now.QuadPart+10*frequency.QuadPart,frequency.QuadPart};
  input.current={reinterpret_cast<uintptr_t>(&fixture),Cancelled};SsFinishedResultV1 result{};
  ss_finished_open_v1(&input,&result);
  if(!KnownNoOwner(result,ERROR_OPERATION_ABORTED,FinishedCauseControl))return false;
  input.current.check=Current;input.original_operation.counter_deadline=now.QuadPart-1;
  ss_finished_open_v1(&input,&result);
  if(!KnownNoOwner(result,ERROR_TIMEOUT,FinishedCauseQpc)||result.status!=DeadlineElapsed)return false;
  input.abi_version=2;ss_finished_open_v1(&input,&result);
  if(result.retired||result.lease||result.primary_error!=ERROR_INVALID_PARAMETER)return false;
  // Inert untrusted permit bytes are never examined on this cancelled branch;
  // no source/intent proof or authority is produced by a no-owner result.
  SsCaptureDispositionV1 inert{};SsFinishedRecoverInputV1 recovery{};recovery.abi_version=1;
  recovery.borrowed_database=input.borrowed_database;recovery.borrowed_metrics=input.borrowed_metrics;
  recovery.original_current_operation={now.QuadPart+10*frequency.QuadPart,frequency.QuadPart};
  recovery.current={reinterpret_cast<uintptr_t>(&fixture),Cancelled};recovery.borrowed_live_recovery_permit=&inert;
  SsFinishedDispositionResultV1 disposition{};ss_finished_recover_disposition_v1(&recovery,&disposition);
  return KnownNoOwner(disposition.state,ERROR_OPERATION_ABORTED,FinishedCauseControl)&&
      !disposition.stages_absent_mask&&disposition.source_state==SourceUncertain&&disposition.metadata_state==MetadataUncertain;
}
bool RejectForeignGeneration(Fixture& fixture) {
  LARGE_INTEGER now{},frequency{};
  if(!QueryPerformanceCounter(&now)||!QueryPerformanceFrequency(&frequency))return false;
  SsFinishedOpenInputV1 input{};input.abi_version=1;
  input.borrowed_database=reinterpret_cast<uintptr_t>(fixture.directory);input.borrowed_metrics=input.borrowed_database;
  input.subject.tag=Desktop;input.subject.epoch[0]=1;input.subject.instance=1;input.subject.generation=1;
  input.build_length=1;input.build[0]='a';input.original_operation={now.QuadPart+10*frequency.QuadPart,frequency.QuadPart};
  input.current={reinterpret_cast<uintptr_t>(&fixture),Current};
  // All-zero expected native IDs disagree with this actual protected root.
  SsFinishedResultV1 observed{};ss_finished_open_v1(&input,&observed);
  if(!observed.lease)return false;
  SsFinishedCallV1 cleanup{};cleanup.abi_version=1;cleanup.phase=Cleanup;cleanup.lease=observed.lease;
  cleanup.original=input.original_operation;cleanup.current=input.current;
  SsFinishedResultV1 retired{};ss_finished_retire_v1(&cleanup,&retired);
  return observed.status==WrongIdentity&&!observed.facts_valid&&observed.live_handles==2&&
      retired.retired&&!retired.lease&&!retired.live_handles&&!retired.live_locks;
}
bool Exercise(Fixture& fixture) {
  auto& api = sf::WindowsFiles(); FixtureControl control;
  if(!NoOwnerCreation(fixture))return false;
  if(!RejectForeignGeneration(fixture))return false;
  if (api.Open(fixture.directory, L"native.stage", sf::OpenMode::CreateStage, fixture.file, control)) return false;
  const std::array<uint8_t, 3> image{'a','b','c'};
  if (api.Write(fixture.file, 0, image, control) || api.Truncate(fixture.file, image.size(), control) || api.Flush(fixture.file)) return false;
  sf::FileInfo info{};
  if (api.Inspect(fixture.file, fixture.directory, info, control) || info.directory || info.bytes != image.size()) return false;
  std::array<uint8_t, 3> read{};
  if (api.Read(fixture.file, 0, read, control) || read != image) return false;
  StoppingControl stopped;
  const std::array<uint8_t,3> replacement{'x','y','z'};
  if(api.Write(fixture.file,0,replacement,stopped)!=ERROR_OPERATION_ABORTED||stopped.calls!=1 ||
      api.Read(fixture.file,0,read,control)||read!=image)return false;
  stopped.calls=0;stopped.stop=2;
  if(api.Inspect(fixture.file,fixture.directory,info,stopped)!=ERROR_OPERATION_ABORTED||stopped.calls!=2)return false;
  stopped.calls=0;
  sf::HashState partial_hash;
  const auto hash_error=sf::BeginHash(partial_hash,stopped);
  const bool adopted=partial_hash.algorithm&&!partial_hash.hash;
  const auto hash_closed=sf::RetireHash(partial_hash);
  if(hash_error!=ERROR_OPERATION_ABORTED||stopped.calls!=2||!adopted||hash_closed||partial_hash.algorithm||partial_hash.hash)return false;
  if (api.Lock(fixture.file, true) || api.Unlock(fixture.file)) return false;
  HANDLE collision = nullptr;
  const auto collision_error = api.Open(fixture.directory, L"native.stage", sf::OpenMode::CreateStage, collision, control);
  // Any real error-returned output is immediately adopted before assertions.
  fixture.inspection = collision;
  if (collision || collision_error != ERROR_ALREADY_EXISTS) return false;
  if (api.Dispose(fixture.file) || api.Close(fixture.file)) return false;
  fixture.file = nullptr;
  const auto absent = api.Open(fixture.directory, L"native.stage", sf::OpenMode::Read, fixture.inspection, control);
  if (absent != ERROR_FILE_NOT_FOUND || fixture.inspection) return false;
  // Characterize the actual pinned upstream report creation style: inherited
  // account ACL, default native label, no explicit security attributes.
  const auto path = fixture.root + L"\\native.stage";
  fixture.file = CreateFileW(path.c_str(), GENERIC_READ | GENERIC_WRITE | DELETE,
      FILE_SHARE_READ, nullptr, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, nullptr);
  if (fixture.file == INVALID_HANDLE_VALUE) { fixture.file = nullptr; return false; }
  const auto protection = api.Inspect(fixture.file, fixture.directory, info, control);
  if (protection) return false;
  // A real explicitly lowered file remains rejected, independently of the
  // inherited account ACL and matching retained inode.
  if (!CloseHandle(fixture.file)) return false;
  fixture.file = CreateFileW(path.c_str(), GENERIC_READ | WRITE_OWNER | WRITE_DAC,
      FILE_SHARE_READ, nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
  if (fixture.file == INVALID_HANDLE_VALUE) { fixture.file = nullptr; return false; }
  if (fixture.descriptor) { if (LocalFree(fixture.descriptor)) return false; fixture.descriptor=nullptr; }
  if (!ConvertStringSecurityDescriptorToSecurityDescriptorW(L"S:(ML;;NW;;;LW)",SDDL_REVISION_1,&fixture.descriptor,nullptr)) return false;
  if (!SetKernelObjectSecurity(fixture.file,LABEL_SECURITY_INFORMATION,fixture.descriptor)) return false;
  if (api.Inspect(fixture.file,fixture.directory,info,control)!=ERROR_ACCESS_DENIED) return false;
  if (api.Close(fixture.file)) return false;
  fixture.file=nullptr;
  if (!DeleteFileW(path.c_str())) return false;
  fixture.file=CreateFileW(path.c_str(),GENERIC_READ|WRITE_DAC,FILE_SHARE_READ,nullptr,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,nullptr);
  if (fixture.file==INVALID_HANDLE_VALUE) {fixture.file=nullptr;return false;}
  if (fixture.descriptor) {if(LocalFree(fixture.descriptor))return false;fixture.descriptor=nullptr;}
  if (!ConvertStringSecurityDescriptorToSecurityDescriptorW(L"D:P(A;;FA;;;WD)",SDDL_REVISION_1,&fixture.descriptor,nullptr) ||
      !SetKernelObjectSecurity(fixture.file,DACL_SECURITY_INFORMATION,fixture.descriptor)) return false;
  return api.Inspect(fixture.file,fixture.directory,info,control)==ERROR_ACCESS_DENIED;
}
}  // namespace
int wmain(int argc, wchar_t** argv) {
  if (argc != 2) return 2;
  Fixture fixture;
  const bool passed = fixture.Start(argv[1]) && Exercise(fixture);
  const bool closed = fixture.Close();
  std::printf("finished Windows file port: %s, checked fixture cleanup: %s\n", passed ? "passed" : "failed", closed ? "passed" : "failed");
  return passed && closed ? 0 : 1;
}
