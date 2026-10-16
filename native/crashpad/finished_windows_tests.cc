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
  sf::Lease* inventory = nullptr;
  bool CloseInventory();
  PSECURITY_DESCRIPTOR descriptor = nullptr;
  LPWSTR sid_text = nullptr;
  std::wstring root;
  const char* failure_step=nullptr;DWORD failure_code=0;
  bool Fail(const char* step,DWORD code=0) {
    if(!failure_step){failure_step=step;failure_code=code;}
    return false;
  }
  bool made = false, made_reports = false, made_attachments = false, made_unknown = false, made_attachment_file = false;
  bool Close() {
    auto& api = sf::WindowsFiles(); bool ok = CloseInventory();
    for (auto slot : {&inspection, &file, &directory, &token}) {
      if (*slot) { if (const auto error=api.Close(*slot)) {Fail("fixture-handle-close",error);ok=false;} else *slot = nullptr; }
    }
    if (made && !directory && !file && !inspection && !inventory) {
      const auto path = root + L"\\native.stage";
      if(!DeleteFileW(path.c_str())){const auto error=GetLastError();if(error!=ERROR_FILE_NOT_FOUND){Fail("fixture-stage-delete",error);ok=false;}}
      if (made_attachments) {
        const auto attachments = root + L"\\attachments";
        if (made_attachment_file) {
          if(!DeleteFileW((attachments+L"\\native.stage").c_str())){Fail("fixture-attachment-delete",GetLastError());ok=false;}else made_attachment_file=false;
        }
        if (made_unknown) {
          if(!RemoveDirectoryW((attachments+L"\\unknown").c_str())){Fail("fixture-unknown-directory-remove",GetLastError());ok=false;}else made_unknown=false;
        }
        if(!RemoveDirectoryW(attachments.c_str())){Fail("fixture-attachments-remove",GetLastError());ok=false;}else made_attachments=false;
      }
      if (made_reports) {
        const auto reports = root + L"\\reports";
        if(!RemoveDirectoryW(reports.c_str())){Fail("fixture-reports-remove",GetLastError());ok=false;}else made_reports=false;
      }
      if(!RemoveDirectoryW(root.c_str())){Fail("fixture-root-remove",GetLastError());ok=false;}else made=false;
    }
    if(descriptor){if(LocalFree(descriptor)){Fail("fixture-descriptor-retire",GetLastError());ok=false;}else descriptor=nullptr;}
    if(sid_text){if(LocalFree(sid_text)){Fail("fixture-sid-text-retire",GetLastError());ok=false;}else sid_text=nullptr;}
    return ok;
  }
  bool Start(const wchar_t* input) {
    root = input;
    constexpr wchar_t prefix[] = L"D:\\SugarSubstitute-Rust\\artifacts\\k1\\finished-native01\\";
    if (!root.starts_with(prefix) || root.find(L"..") != std::wstring::npos ||
        GetFileAttributesW(root.c_str()) != INVALID_FILE_ATTRIBUTES) return false;
    if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token)) return Fail("fixture-token-open",GetLastError());
    std::array<uint32_t, 256> buffer{}; DWORD needed = 0;
    if (!GetTokenInformation(token, TokenUser, buffer.data(), static_cast<DWORD>(sizeof(buffer)), &needed)) return Fail("fixture-token-query",GetLastError());
    if(needed>sizeof(buffer))return Fail("fixture-token-bound");
    const auto user = reinterpret_cast<TOKEN_USER*>(buffer.data());
    if (!ConvertSidToStringSidW(user->User.Sid, &sid_text)) return Fail("fixture-sid-text",GetLastError());
    const std::wstring text = L"O:" + std::wstring(sid_text) + L"D:P(A;OICI;FA;;;" + sid_text + L")(A;OICI;FA;;;SY)S:(ML;;NW;;;ME)";
    if (!ConvertStringSecurityDescriptorToSecurityDescriptorW(text.c_str(), SDDL_REVISION_1, &descriptor, nullptr)) return Fail("fixture-security-descriptor",GetLastError());
    SECURITY_ATTRIBUTES attributes{sizeof(attributes), descriptor, FALSE};
    if (!CreateDirectoryW(root.c_str(), &attributes)) return Fail("fixture-root-create",GetLastError());
    made = true;
    directory = CreateFileW(root.c_str(), FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | READ_CONTROL | SYNCHRONIZE,
        FILE_SHARE_READ | FILE_SHARE_WRITE, nullptr, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT, nullptr);
    if (directory == INVALID_HANDLE_VALUE) { const auto error=GetLastError();directory=nullptr;return Fail("fixture-root-open",error); }
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
bool Fixture::CloseInventory() {
  if (!inventory) return true;
  if (!inventory->cleanup_latched) {
    int64_t now = 0, frequency = 0;
    if (const auto error=sf::WindowsFiles().Clock(now, frequency)) return Fail("inventory-cleanup-clock",error);
    inventory->in_cleanup = true;
    inventory->cleanup = {now + 10 * frequency, frequency};
    inventory->cleanup_latched = true;
  }
  sf::Invocation cleanup(*inventory, {reinterpret_cast<uintptr_t>(this), Current});
  if (!cleanup.Retire()) return Fail("inventory-checked-retirement",inventory->primary);
  delete inventory; inventory = nullptr;
  return true;
}
bool KnownNoOwner(const SsFinishedResultV1& result, DWORD cause, SsFinishedCauseOriginV1 origin) {
  return result.abi_version==1&&result.retired==1&&!result.lease&&!result.live_handles&&!result.live_locks&&
      !result.facts_valid&&!result.actual_read_bytes&&!result.actual_reclaimed_bytes&&result.primary_error==cause&&result.primary_origin==origin;
}
bool NoOwnerCreation(Fixture& fixture) {
  LARGE_INTEGER now{},frequency{};
  if(!QueryPerformanceCounter(&now)||!QueryPerformanceFrequency(&frequency))return fixture.Fail("no-owner-operation-clock",GetLastError());
  SsFinishedOpenInputV1 input{};input.abi_version=1;
  input.borrowed_database=reinterpret_cast<uintptr_t>(fixture.directory);input.borrowed_metrics=input.borrowed_database;
  input.subject.tag=Desktop;input.subject.epoch[0]=1;input.subject.instance=1;input.subject.generation=1;
  input.build_length=1;input.build[0]='a';input.original_operation={now.QuadPart+10*frequency.QuadPart,frequency.QuadPart};
  input.current={reinterpret_cast<uintptr_t>(&fixture),Cancelled};SsFinishedResultV1 result{};
  ss_finished_open_v1(&input,&result);
  if(!KnownNoOwner(result,ERROR_OPERATION_ABORTED,FinishedCauseControl))return fixture.Fail("cancelled-open-no-owner",result.primary_error);
  input.current.check=Current;input.original_operation.counter_deadline=now.QuadPart-1;
  ss_finished_open_v1(&input,&result);
  if(!KnownNoOwner(result,ERROR_TIMEOUT,FinishedCauseQpc)||result.status!=DeadlineElapsed)return fixture.Fail("expired-open-no-owner",result.primary_error);
  input.abi_version=2;ss_finished_open_v1(&input,&result);
  if(result.retired||result.lease||result.primary_error!=ERROR_INVALID_PARAMETER)return fixture.Fail("unsupported-open-version",result.primary_error);
  // Inert untrusted permit bytes are never examined on this cancelled branch;
  // no source/intent proof or authority is produced by a no-owner result.
  SsCaptureDispositionV1 inert{};SsFinishedRecoverInputV1 recovery{};recovery.abi_version=1;
  recovery.borrowed_database=input.borrowed_database;recovery.borrowed_metrics=input.borrowed_metrics;
  recovery.original_current_operation={now.QuadPart+10*frequency.QuadPart,frequency.QuadPart};
  recovery.current={reinterpret_cast<uintptr_t>(&fixture),Cancelled};recovery.borrowed_live_recovery_permit=&inert;
  SsFinishedDispositionResultV1 disposition{};ss_finished_recover_disposition_v1(&recovery,&disposition);
  return (KnownNoOwner(disposition.state,ERROR_OPERATION_ABORTED,FinishedCauseControl)&&
      !disposition.stages_absent_mask&&disposition.source_state==SourceUncertain&&disposition.metadata_state==MetadataUncertain)||
      fixture.Fail("cancelled-recovery-no-owner",disposition.state.primary_error);
}
bool RejectForeignGeneration(Fixture& fixture) {
  LARGE_INTEGER now{},frequency{};
  if(!QueryPerformanceCounter(&now)||!QueryPerformanceFrequency(&frequency))return fixture.Fail("foreign-generation-operation-clock",GetLastError());
  SsFinishedOpenInputV1 input{};input.abi_version=1;
  input.borrowed_database=reinterpret_cast<uintptr_t>(fixture.directory);input.borrowed_metrics=input.borrowed_database;
  input.subject.tag=Desktop;input.subject.epoch[0]=1;input.subject.instance=1;input.subject.generation=1;
  input.build_length=1;input.build[0]='a';input.original_operation={now.QuadPart+10*frequency.QuadPart,frequency.QuadPart};
  input.current={reinterpret_cast<uintptr_t>(&fixture),Current};
  // All-zero expected native IDs disagree with this actual protected root.
  SsFinishedResultV1 observed{};ss_finished_open_v1(&input,&observed);
  if(!observed.lease)return fixture.Fail("foreign-generation-issued-owner",observed.primary_error);
  SsFinishedCallV1 cleanup{};cleanup.abi_version=1;cleanup.phase=Cleanup;cleanup.lease=observed.lease;
  cleanup.original=input.original_operation;cleanup.current=input.current;
  SsFinishedResultV1 retired{};ss_finished_retire_v1(&cleanup,&retired);
  return (observed.status==WrongIdentity&&!observed.facts_valid&&observed.live_handles==2&&
      retired.retired&&!retired.lease&&!retired.live_handles&&!retired.live_locks)||
      fixture.Fail("foreign-generation-checked-retirement",retired.primary_error);
}
bool DirectoryInspection(Fixture& fixture) {
  auto& api = sf::WindowsFiles(); FixtureControl control;
  sf::FileInfo anchor{}, reopened{};
  const auto anchor_error=api.Inspect(fixture.directory, fixture.directory, anchor, control);
  if(anchor_error)return fixture.Fail("directory-anchor-inspect",anchor_error);
  const auto reports = fixture.root + L"\\reports";
  if (!CreateDirectoryW(reports.c_str(), nullptr)) return fixture.Fail("reports-create",GetLastError());
  fixture.made_reports = true;
  // Exercise the actual relative NT directory open on an inherited child,
  // matching Crashpad's report-directory creation and retained parent ACL.
  const auto open_error=api.Open(fixture.directory,L"reports",sf::OpenMode::Directory,fixture.inspection,control);
  if(open_error)return fixture.Fail("reports-relative-open",open_error);
  const auto inspected = api.Inspect(fixture.inspection, fixture.directory, reopened, control);
  const auto closed = api.Close(fixture.inspection);
  if (!closed) fixture.inspection = nullptr;
  if(inspected)return fixture.Fail("reports-relative-inspect",inspected);
  if(closed)return fixture.Fail("reports-relative-close",closed);
  return reopened.directory&&reopened.identity!=anchor.identity ? true : fixture.Fail("reports-distinct-directory-facts");
}
bool DirectorySelfScan(Fixture& fixture) {
  auto& api=sf::WindowsFiles();FixtureControl control;
  const auto check=[&](const char* step,DWORD error){return !error||fixture.Fail(step,error);};
  if(!check("self-first-open",api.Open(fixture.directory,L".",sf::OpenMode::Directory,fixture.inspection,control))||
      !check("self-second-open",api.Open(fixture.directory,L".",sf::OpenMode::Directory,fixture.file,control)))return false;
  sf::FileInfo anchor{},first{},second{};
  bool passed=check("self-anchor-inspect",api.Inspect(fixture.directory,fixture.directory,anchor,control))&&
      check("self-first-inspect",api.Inspect(fixture.inspection,fixture.directory,first,control))&&
      check("self-second-inspect",api.Inspect(fixture.file,fixture.directory,second,control))&&
      ((first.directory&&second.directory&&first.identity==anchor.identity&&second.identity==anchor.identity)||fixture.Fail("self-same-directory-identities"));
  alignas(FILE_ID_BOTH_DIR_INFO) std::array<uint8_t,65536> page{};
  alignas(FILE_ID_BOTH_DIR_INFO) std::array<uint8_t,65537> invalid_page{};
  uint32_t entries=0;uint64_t bytes=0;bool end=false,complete=false;
  const auto rejected=[&](const char* step,std::span<uint8_t> buffer){
    entries=99;bytes=99;end=true;complete=true;
    const auto error=api.Scan(fixture.inspection,sf::ScanRole::Database,false,buffer,entries,bytes,end,complete);
    return (error==ERROR_INVALID_PARAMETER&&!entries&&!bytes&&!end&&!complete)||fixture.Fail(step,error);
  };
  passed=passed&&rejected("scan-misaligned-page-rejected",std::span(invalid_page).subspan(1));
  passed=passed&&rejected("scan-undersized-page-rejected",std::span(invalid_page).first(sizeof(FILE_ID_BOTH_DIR_INFO)-1));
  // The first genuine page below still contains reports: rejected storage has
  // not consumed the actual directory handle's independent enumeration cursor.
  const auto scan=[&](const char* step,HANDLE directory,uint32_t expected,bool expected_end){
    const auto error=api.Scan(directory,sf::ScanRole::Database,false,page,entries,bytes,end,complete);
    if(error)return fixture.Fail(step,error);
    if(entries!=expected||bytes||end!=expected_end||!complete){
      std::printf("finished Windows scan facts: phase=%s entries=%lu bytes=%llu end=%u complete=%u\n",step,
          static_cast<unsigned long>(entries),static_cast<unsigned long long>(bytes),unsigned(end),unsigned(complete));
      return fixture.Fail(step);
    }
    return true;
  };
  passed=passed&&scan("self-first-page",fixture.inspection,1,false);
  passed=passed&&scan("self-first-eof",fixture.inspection,0,true);
  passed=passed&&scan("self-second-page",fixture.file,1,false);
  passed=passed&&scan("self-second-eof",fixture.file,0,true);
  const auto first_closed=api.Close(fixture.inspection);if(!first_closed)fixture.inspection=nullptr;
  const bool first_retired=check("self-first-close",first_closed);
  const auto second_closed=api.Close(fixture.file);if(!second_closed)fixture.file=nullptr;
  const bool second_retired=check("self-second-close",second_closed);
  return passed&&first_retired&&second_retired;
}

bool AttachmentOwner(Fixture& fixture, const char* scenario, bool empty, uint32_t expected_entries, uint64_t expected_bytes) {
  auto& api = sf::WindowsFiles();
  if (fixture.inventory) return fixture.Fail("attachment-existing-inventory");
  fixture.inventory = sf::NativeLeaseAllocation(api);
  if (!fixture.inventory) return fixture.Fail("attachment-owner-allocation",ERROR_NOT_ENOUGH_MEMORY);
  auto& owner = *fixture.inventory;
  int64_t now = 0, frequency = 0;
  if (const auto error=api.Clock(now, frequency)) return fixture.Fail("attachment-operation-clock",error);
  owner.operation = {now + 10 * frequency, frequency};
  owner.facts.inventory_complete = 1;
  sf::Invocation invocation(owner, {reinterpret_cast<uintptr_t>(&fixture), Current});
  const auto duplicated = api.Duplicate(fixture.directory, owner.handles[sf::Database]);
  const bool inventoried = !duplicated && sf::InventoryAttachments(invocation);
  const bool passed = !duplicated && (empty ? inventoried && !owner.primary :
      !inventoried && owner.primary == ERROR_NOT_SUPPORTED && owner.handles[sf::ScanDirectory]) &&
      owner.facts.observed_entries == expected_entries && owner.facts.observed_temporary_bytes == expected_bytes;
  if(!passed) {
    std::printf("finished Windows attachment facts: case=%s duplicate=%lu inventoried=%u primary=%lu origin=%u status=%u scan_retained=%u entries=%lu expected_entries=%lu bytes=%llu expected_bytes=%llu complete=%u\n",
        scenario,static_cast<unsigned long>(duplicated),unsigned(inventoried),static_cast<unsigned long>(owner.primary),
        unsigned(owner.primary_origin),unsigned(owner.status),unsigned(owner.handles[sf::ScanDirectory]!=nullptr),
        static_cast<unsigned long>(owner.facts.observed_entries),static_cast<unsigned long>(expected_entries),
        static_cast<unsigned long long>(owner.facts.observed_temporary_bytes),static_cast<unsigned long long>(expected_bytes),unsigned(owner.facts.inventory_complete));
    fixture.Fail(scenario,duplicated?duplicated:owner.primary);
  }
  const bool retired = fixture.CloseInventory();
  return passed && retired;
}
bool AttachmentInventory(Fixture& fixture) {
  const auto attachments = fixture.root + L"\\attachments";
  if (!CreateDirectoryW(attachments.c_str(), nullptr)) return fixture.Fail("attachments-create",GetLastError());
  fixture.made_attachments = true;
  if (!AttachmentOwner(fixture,"attachment-empty",true,0,0)) return false;
  fixture.file = CreateFileW((attachments + L"\\native.stage").c_str(), GENERIC_WRITE,
      FILE_SHARE_READ, nullptr, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, nullptr);
  if (fixture.file == INVALID_HANDLE_VALUE) {const auto error=GetLastError();fixture.file=nullptr;return fixture.Fail("attachment-file-create",error);}
  fixture.made_attachment_file = true;
  const std::array<uint8_t,3> payload{'a','b','c'}; DWORD written = 0;
  if (!WriteFile(fixture.file,payload.data(),static_cast<DWORD>(payload.size()),&written,nullptr)) return fixture.Fail("attachment-file-write",GetLastError());
  if(written!=payload.size())return fixture.Fail("attachment-file-write-count");
  if(const auto error=sf::WindowsFiles().Close(fixture.file))return fixture.Fail("attachment-file-close",error);
  fixture.file = nullptr;
  if (!AttachmentOwner(fixture,"attachment-file-rejected",false,1,payload.size())) return false;
  if (!DeleteFileW((attachments + L"\\native.stage").c_str())) return fixture.Fail("attachment-file-delete",GetLastError());
  fixture.made_attachment_file = false;
  if (!CreateDirectoryW((attachments + L"\\unknown").c_str(),nullptr)) return fixture.Fail("attachment-unknown-directory-create",GetLastError());
  fixture.made_unknown = true;
  return AttachmentOwner(fixture,"attachment-unknown-directory-rejected",false,1,0);
}
bool Exercise(Fixture& fixture) {
  auto& api = sf::WindowsFiles(); FixtureControl control;
  const auto check=[&](const char* step,DWORD error){return !error||fixture.Fail(step,error);};
  if(!DirectoryInspection(fixture))return fixture.Fail("directory-inspection");
  if(!DirectorySelfScan(fixture))return fixture.Fail("independent-directory-scan");
  if(!AttachmentInventory(fixture))return fixture.Fail("attachment-inventory");
  if(!NoOwnerCreation(fixture))return fixture.Fail("no-owner-creation");
  if(!RejectForeignGeneration(fixture))return fixture.Fail("foreign-generation");
  const auto stage_error=api.Open(fixture.directory,L"native.stage",sf::OpenMode::CreateStage,fixture.file,control);
  if(stage_error)return fixture.Fail("native-stage-create",stage_error);
  const std::array<uint8_t, 3> image{'a','b','c'};
  if(!check("stage-write",api.Write(fixture.file,0,image,control))||
      !check("stage-truncate",api.Truncate(fixture.file,image.size(),control))||!check("stage-flush",api.Flush(fixture.file)))return false;
  sf::FileInfo info{};
  if(!check("stage-inspect",api.Inspect(fixture.file,fixture.directory,info,control)))return false;
  if(info.directory||info.bytes!=image.size())return fixture.Fail("stage-file-facts");
  std::array<uint8_t, 3> read{};
  if(!check("stage-read",api.Read(fixture.file,0,read,control)))return false;
  if(read!=image)return fixture.Fail("stage-read-content");
  StoppingControl stopped;
  const std::array<uint8_t,3> replacement{'x','y','z'};
  if(api.Write(fixture.file,0,replacement,stopped)!=ERROR_OPERATION_ABORTED||stopped.calls!=1 ||
      api.Read(fixture.file,0,read,control)||read!=image)return fixture.Fail("cancelled-write-keeps-content");
  stopped.calls=0;stopped.stop=2;
  if(api.Inspect(fixture.file,fixture.directory,info,stopped)!=ERROR_OPERATION_ABORTED||stopped.calls!=2)return fixture.Fail("cancelled-inspect");
  stopped.calls=0;
  sf::HashState partial_hash;
  const auto hash_error=sf::BeginHash(partial_hash,stopped);
  const bool adopted=partial_hash.algorithm&&!partial_hash.hash;
  const auto hash_closed=sf::RetireHash(partial_hash);
  if(hash_error!=ERROR_OPERATION_ABORTED||stopped.calls!=2||!adopted||hash_closed||partial_hash.algorithm||partial_hash.hash)return fixture.Fail("cancelled-hash-partial-retirement",hash_error);
  if(!check("stage-lock",api.Lock(fixture.file,true))||!check("stage-unlock",api.Unlock(fixture.file)))return false;
  HANDLE collision = nullptr;
  const auto collision_error = api.Open(fixture.directory, L"native.stage", sf::OpenMode::CreateStage, collision, control);
  // Any real error-returned output is immediately adopted before assertions.
  fixture.inspection = collision;
  if(collision||collision_error!=ERROR_ALREADY_EXISTS)return fixture.Fail("stage-create-collision",collision_error);
  if(!check("stage-dispose",api.Dispose(fixture.file))||!check("stage-close",api.Close(fixture.file)))return false;
  fixture.file = nullptr;
  const auto absent = api.Open(fixture.directory, L"native.stage", sf::OpenMode::Read, fixture.inspection, control);
  if(absent!=ERROR_FILE_NOT_FOUND||fixture.inspection)return fixture.Fail("stage-checked-absence",absent);
  // Characterize the actual pinned upstream report creation style: inherited
  // account ACL, default native label, no explicit security attributes.
  const auto path = fixture.root + L"\\native.stage";
  fixture.file = CreateFileW(path.c_str(), GENERIC_READ | GENERIC_WRITE | DELETE,
      FILE_SHARE_READ, nullptr, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, nullptr);
  if (fixture.file == INVALID_HANDLE_VALUE) {const auto error=GetLastError();fixture.file=nullptr;return fixture.Fail("inherited-file-create",error);}
  const auto protection = api.Inspect(fixture.file, fixture.directory, info, control);
  if(protection)return fixture.Fail("inherited-file-protection",protection);
  // A real explicitly lowered file remains rejected, independently of the
  // inherited account ACL and matching retained inode.
  if (!CloseHandle(fixture.file)) return fixture.Fail("inherited-file-close",GetLastError());
  fixture.file = CreateFileW(path.c_str(), GENERIC_READ | WRITE_OWNER | WRITE_DAC,
      FILE_SHARE_READ, nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
  if (fixture.file == INVALID_HANDLE_VALUE) {const auto error=GetLastError();fixture.file=nullptr;return fixture.Fail("low-label-file-open",error);}
  if (fixture.descriptor) {if(LocalFree(fixture.descriptor))return fixture.Fail("low-label-descriptor-retire",GetLastError());fixture.descriptor=nullptr;}
  if (!ConvertStringSecurityDescriptorToSecurityDescriptorW(L"S:(ML;;NW;;;LW)",SDDL_REVISION_1,&fixture.descriptor,nullptr)) return fixture.Fail("low-label-descriptor-create",GetLastError());
  if (!SetKernelObjectSecurity(fixture.file,LABEL_SECURITY_INFORMATION,fixture.descriptor)) return fixture.Fail("low-label-apply",GetLastError());
  const auto low_label=api.Inspect(fixture.file,fixture.directory,info,control);
  if(low_label!=ERROR_ACCESS_DENIED)return fixture.Fail("low-label-rejected",low_label);
  if(const auto error=api.Close(fixture.file))return fixture.Fail("low-label-file-close",error);
  fixture.file=nullptr;
  if (!DeleteFileW(path.c_str())) return fixture.Fail("low-label-file-delete",GetLastError());
  fixture.file=CreateFileW(path.c_str(),GENERIC_READ|WRITE_DAC,FILE_SHARE_READ,nullptr,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,nullptr);
  if(fixture.file==INVALID_HANDLE_VALUE){const auto error=GetLastError();fixture.file=nullptr;return fixture.Fail("world-dacl-file-create",error);}
  if(fixture.descriptor){if(LocalFree(fixture.descriptor))return fixture.Fail("world-dacl-descriptor-retire",GetLastError());fixture.descriptor=nullptr;}
  if(!ConvertStringSecurityDescriptorToSecurityDescriptorW(L"D:P(A;;FA;;;WD)",SDDL_REVISION_1,&fixture.descriptor,nullptr))return fixture.Fail("world-dacl-descriptor-create",GetLastError());
  if(!SetKernelObjectSecurity(fixture.file,DACL_SECURITY_INFORMATION,fixture.descriptor))return fixture.Fail("world-dacl-apply",GetLastError());
  const auto world=api.Inspect(fixture.file,fixture.directory,info,control);
  return world==ERROR_ACCESS_DENIED ? true : fixture.Fail("world-dacl-rejected",world);
}
}  // namespace
int wmain(int argc, wchar_t** argv) {
  if (argc != 2) return 2;
  Fixture fixture;
  const bool passed = fixture.Start(argv[1]) && Exercise(fixture);
  const bool closed = fixture.Close();
  std::printf("finished Windows file port: %s, checked fixture cleanup: %s\n", passed ? "passed" : "failed", closed ? "passed" : "failed");
  if(!passed||!closed)std::printf("finished Windows first failure: phase=%s error=%lu\n",
      fixture.failure_step?fixture.failure_step:"fixture-start-or-remaining-check",static_cast<unsigned long>(fixture.failure_code));
  return passed && closed ? 0 : 1;
}
