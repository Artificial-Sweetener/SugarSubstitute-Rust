// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "finished_report.h"
#include <cstdio>
#include <cstring>
#include <map>
#include <memory>
#include <string>
#include <vector>

namespace sf = sugarsubstitute::finished;
namespace {
using Bytes = std::vector<uint8_t>;
void Put(Bytes& bytes, size_t offset, uint32_t value) {
  for (size_t i = 0; i < 4; ++i) bytes[offset + i] = static_cast<uint8_t>(value >> (8 * i));
}
struct File { Bytes bytes; uint8_t inode = 0; bool directory = false, pending = false, present = true; };
class Fake final : public sf::FileApi {
 public:
  std::map<std::wstring, File> files;
  std::map<HANDLE, std::wstring> handles;
  uintptr_t next_handle = 20;
  uint8_t next_inode = 30;
  int64_t now = 1;
  uint32_t effects = 0, fail_effect = 0, late_effect = 0, close_fail = 0;
  bool partial_open = false, lock_busy = false, incomplete_scan = false;
  uint32_t failure_code=9911, failed_control_effect=0;bool failed_control=false;
  uint32_t allocations=0;bool fail_allocation=false,stop_after_allocation=false;
  uint32_t cancel_effect=0; bool cancelled=false;
  DWORD Effect() noexcept { ++effects; if(effects==cancel_effect)cancelled=true;if(effects==failed_control_effect)failed_control=true; if (effects == late_effect) now = 100; return effects == fail_effect ? failure_code : 0; }
  HANDLE Adopt(const std::wstring& name) { const auto handle = reinterpret_cast<HANDLE>(next_handle++); handles[handle] = name; return handle; }
  DWORD Clock(int64_t& counter, int64_t& frequency) noexcept override { counter = now; frequency = 1; return 0; }
  DWORD Duplicate(HANDLE input, HANDLE& output) noexcept override {
    output = Adopt(input == reinterpret_cast<HANDLE>(1) ? L"db" : L"metrics"); return Effect();
  }
  DWORD Open(HANDLE parent, const wchar_t* leaf, sf::OpenMode mode, HANDLE& output, sf::ParseControl&) noexcept override {
    const auto prefix = handles[parent]; const auto name = std::wstring(leaf) == L"." ? prefix : prefix + L"/" + leaf;
    const auto error = Effect();
    if (error && !partial_open) return error;
    auto found = files.find(name);
    if (mode == sf::OpenMode::CreateStage) {
      if (found != files.end() && found->second.present) return ERROR_FILE_EXISTS;
      files[name] = {{}, next_inode++, false, false, true};
    } else if (found == files.end() || !found->second.present) return ERROR_FILE_NOT_FOUND;
    output = Adopt(name); return error;
  }
  DWORD Inspect(HANDLE handle, HANDLE, sf::FileInfo& info, sf::ParseControl&) noexcept override {
    const auto error = Effect(); if (error) return error;
    const auto& file = files[handles[handle]];
    info.identity.fill(file.inode); info.bytes = file.bytes.size(); info.directory = file.directory; return 0;
  }
  DWORD Read(HANDLE handle, uint64_t offset, std::span<uint8_t> output, sf::ParseControl&) noexcept override {
    const auto error = Effect(); if (error) return error;
    auto& bytes = files[handles[handle]].bytes;
    if (offset > bytes.size() || output.size() > bytes.size() - offset) return ERROR_HANDLE_EOF;
    std::copy_n(bytes.begin() + static_cast<size_t>(offset), output.size(), output.begin()); return 0;
  }
  DWORD Write(HANDLE handle, uint64_t offset, std::span<const uint8_t> image, sf::ParseControl&) noexcept override {
    const auto error = Effect(); if (error) return error;
    auto& bytes = files[handles[handle]].bytes;
    if (offset + image.size() > bytes.size()) bytes.resize(static_cast<size_t>(offset) + image.size());
    std::copy(image.begin(), image.end(), bytes.begin() + static_cast<size_t>(offset)); return 0;
  }
  DWORD Truncate(HANDLE handle, uint64_t bytes, sf::ParseControl&) noexcept override {
    const auto error = Effect(); if (error) return error;
    files[handles[handle]].bytes.resize(static_cast<size_t>(bytes)); return 0;
  }
  DWORD Flush(HANDLE) noexcept override { return Effect(); }
  DWORD Lock(HANDLE, bool) noexcept override { return lock_busy ? ERROR_LOCK_VIOLATION : Effect(); }
  DWORD Unlock(HANDLE) noexcept override { return Effect(); }
  DWORD Dispose(HANDLE handle) noexcept override {
    const auto error = Effect(); if (error) return error;
    files[handles[handle]].pending = true; return 0;
  }
  DWORD Close(HANDLE handle) noexcept override {
    if (close_fail) return close_fail;
    const auto error = Effect(); if (error) return error;
    const auto name = handles[handle]; handles.erase(handle);
    if (files[name].pending && std::none_of(handles.begin(), handles.end(), [&](const auto& pair) { return pair.second == name; })) files[name].present = false;
    return 0;
  }
  DWORD Scan(HANDLE, sf::ScanRole, bool, std::span<uint8_t>, uint32_t& entries, uint64_t& bytes, bool& end, bool& complete) noexcept override {
    entries = 0; bytes = 0; end = true; complete=!incomplete_scan; return Effect();
  }
  DWORD HashBegin(sf::HashState& state, sf::ParseControl& control) noexcept override { const auto e = Effect(); return e ? e : sf::BeginHash(state, control); }
  DWORD HashUpdate(sf::HashState& state, std::span<const uint8_t> bytes) noexcept override { const auto e = Effect(); return e ? e : sf::UpdateHash(state, bytes); }
  DWORD HashFinish(sf::HashState& state, std::span<uint8_t, 32> digest) noexcept override { const auto e = Effect(); return e ? e : sf::FinishHash(state, digest); }
  DWORD HashRetire(sf::HashState& state, bool hash) noexcept override { const auto e = Effect(); return e ? e : sf::RetireHashPart(state, hash); }
};
constexpr uint8_t native_uuid[16]{0x78,0x56,0x34,0x12,0xab,0x90,0xef,0xcd,0x11,0x22,0x33,0x44,0x55,0x66,0x77,0x88};
constexpr uint8_t uuid[16]{0x12,0x34,0x56,0x78,0x90,0xab,0xcd,0xef,0x11,0x22,0x33,0x44,0x55,0x66,0x77,0x88};
Bytes Metadata() {
  Bytes image(80); Put(image,0,0x43504144); Put(image,4,1); Put(image,8,1);
  std::copy_n(native_uuid,16,image.begin()+16);
  const std::string name = "12345678-90ab-cdef-1122-334455667788.dmp";
  image.insert(image.end(),name.begin(),name.end());image.push_back(0);
  Put(image,36,41);image.push_back(0); return image;
}
struct Builder {
  Bytes bytes;
  uint32_t Add(size_t size) { const auto offset=static_cast<uint32_t>(bytes.size());bytes.resize(bytes.size()+size);return offset; }
  uint32_t Text(const std::string& text) {const auto off=Add(text.size()+5);Put(bytes,off,static_cast<uint32_t>(text.size()));std::copy(text.begin(),text.end(),bytes.begin()+off+4);return off;}
  uint32_t Dict(const std::vector<std::pair<std::string,std::string>>& pairs) {const auto off=Add(4+pairs.size()*8);Put(bytes,off,static_cast<uint32_t>(pairs.size()));for(size_t i=0;i<pairs.size();++i){const auto key=Text(pairs[i].first),value=Text(pairs[i].second);Put(bytes,off+4+i*8,key);Put(bytes,off+8+i*8,value);}return off;}
};
Bytes Dump() {
  Builder b;b.Add(32);const auto dir=b.Add(24),mods=b.Add(112),cp=b.Add(64),links=b.Add(16),info=b.Add(28);
  std::string coordinator;for(size_t i=0;i<32;++i)coordinator+="12";for(size_t i=0;i<16;++i)coordinator+="ab";
  const auto dict=b.Dict({{"product","SugarSubstitute"},{"build","b7"},{"role","supervisor"},{"coordinator",coordinator},{"subject","s1:"+coordinator}});
  Put(b.bytes,0,0x504d444d);Put(b.bytes,4,0xa793);Put(b.bytes,8,2);Put(b.bytes,12,dir);
  Put(b.bytes,dir,4);Put(b.bytes,dir+4,112);Put(b.bytes,dir+8,mods);Put(b.bytes,dir+12,0x43500001);Put(b.bytes,dir+16,64);Put(b.bytes,dir+20,cp);
  Put(b.bytes,mods,1);Put(b.bytes,cp,1);std::copy_n(native_uuid,16,b.bytes.begin()+cp+4);
  Put(b.bytes,cp+44,16);Put(b.bytes,cp+48,links);Put(b.bytes,links,1);Put(b.bytes,links+8,28);Put(b.bytes,links+12,info);
  Put(b.bytes,info,1);Put(b.bytes,info+12,44);Put(b.bytes,info+16,dict);return b.bytes;
}
struct TestControl { Fake& api; bool cleanup=false; uint32_t calls=0; };
SsFinishedControlStatusV1 __cdecl Current(uintptr_t state,uint32_t* cause) noexcept {
  auto& control=*reinterpret_cast<TestControl*>(state);++control.calls;
  if(control.api.failed_control&&!control.cleanup){*cause=ERROR_GEN_FAILURE;return FinishedControlFailed;}
  *cause=control.api.cancelled&&!control.cleanup?ERROR_OPERATION_ABORTED:0;
  return *cause?FinishedControlCancelled:FinishedControlCurrent;
}
struct Fixture {
  Fake api; TestControl main{api,false},cleanup{api,true}; std::unique_ptr<sf::Lease> owner;
  Fixture():owner(std::make_unique<sf::Lease>(api)) {
    api.files[L"db"]={{},1,true};api.files[L"metrics"]={{},2,true};api.files[L"db/reports"]={{},3,true};
    api.files[L"db/metadata"]={Metadata(),4};api.files[L"db/reports/12345678-90ab-cdef-1122-334455667788.dmp"]={Dump(),5};
    auto& input=owner->expected;input.borrowed_database=1;input.borrowed_metrics=2;
    std::fill_n(input.expected_database_identity,24,uint8_t{1});std::fill_n(input.expected_metrics_identity,24,uint8_t{2});
    input.subject.tag=Supervisor;std::fill_n(input.subject.namespace_id,32,uint8_t{0x12});std::fill_n(input.subject.epoch,16,uint8_t{0xab});
    input.build_length=2;memcpy(input.build,"b7",2);owner->operation={100,1};
  }
  ~Fixture() {if(!owner)return;api.fail_effect=api.late_effect=api.close_fail=0;api.now=std::max(api.now,owner->last_counter);owner->in_cleanup=true;owner->cleanup={1000,1};owner->cleanup_latched=true;sf::Invocation invocation(*owner,CleanupSource());invocation.Retire();}
  SsFinishedControlSourceV1 Source(){return {reinterpret_cast<uintptr_t>(&main),&Current};}
  SsFinishedControlSourceV1 CleanupSource(){return {reinterpret_cast<uintptr_t>(&cleanup),&Current};}
  bool Query(){sf::Invocation invocation(*owner,Source());return sf::Query(invocation);}
  bool Prepare(){sf::Invocation invocation(*owner,Source());return sf::Prepare(invocation,Key());}
  bool PrepareKey(const SsLogicalStoreOperationV1& key){sf::Invocation invocation(*owner,Source());return sf::Prepare(invocation,key);}
  bool Disposition(const SsCaptureDispositionV1& permit){sf::Invocation invocation(*owner,Source());return sf::Disposition(invocation,permit);}
  bool StageNames(const SsLogicalStoreOperationV1& key){sf::Invocation invocation(*owner,Source());return sf::StageNames(invocation,key);}
  bool Retire(){sf::Invocation invocation(*owner,CleanupSource());return invocation.Retire();}
  bool Recover(const SsFinishedRecoverInputV1& input){sf::Invocation invocation(*owner,Source());return sf::Recover(invocation,input);}
  SsLogicalStoreOperationV1 Key() {SsLogicalStoreOperationV1 k{};memcpy(k.namespace_id,owner->expected.subject.namespace_id,32);memcpy(k.epoch,owner->expected.subject.epoch,16);k.id=1;return k;}
  SsCaptureDispositionV1 Permit(bool recovery=false) {
    SsCaptureDispositionV1 p{};p.prepared=owner->preparation;p.disposition=1;p.incident_revision=1;p.has_payload=1;
    p.committed_payload=p.prepared.source;p.committed_summary.bytes=100;p.committed_intent.bytes=100;
    memset(p.committed_summary.identity,10,24);memset(p.committed_summary.sha256,11,32);memset(p.committed_intent.identity,12,24);memset(p.committed_intent.sha256,13,32);memset(p.canonical_plan_sha256,13,32);
    p.current_operation=Key();p.authorization=recovery?RecoveryDisposition:NormalDisposition;if(recovery)p.current_operation.id=2;return p;
  }
};
bool Normal() {Fixture f;if(!f.Query()||!f.Prepare())return false;const auto p=f.Permit();return f.Disposition(p)&&f.api.handles.empty()&&f.owner->absent_mask==3&&f.owner->reclaimed==p.prepared.source.bytes&&f.owner->metadata_state==CandidateImage;}
bool BoundsAndWrongKey() {Fixture f;if(!f.Query())return false;auto k=f.Key();k.epoch[0]^=1;const auto before=f.api.effects;return !f.PrepareKey(k)&&f.api.effects==before&&f.api.handles.size()==5;}
bool PartialOutput() {Fixture f;f.api.fail_effect=7;f.api.partial_open=true;const bool ok=!f.Query()&&f.owner->primary==9911&&!f.api.handles.empty();return ok;}
bool LatePartial() {Fixture f;f.api.late_effect=1;return !f.Query()&&f.owner->primary==ERROR_TIMEOUT&&f.api.handles.size()==1;}
bool LockBusy() {Fixture f;f.api.lock_busy=true;return !f.Query()&&f.owner->status==Busy&&f.api.handles.size()==4;}
bool FailedCloseRetains() {Fixture f;if(!f.Query())return false;f.api.close_fail=9922;f.owner->in_cleanup=true;f.owner->cleanup={1000,1};return !f.Retire()&&f.owner->primary==9922&&f.api.handles.size()==5;}
bool Collision() {Fixture f;if(!f.Query()||!f.StageNames(f.Key()))return false;const auto name=L"db/"+std::wstring(f.owner->stage_names[0].data());f.api.files[name]={{1,2},90};return !f.Prepare()&&f.api.files[name].bytes==Bytes({1,2})&&f.api.files[L"db/metadata"].bytes==Metadata();}
bool ThirdImage() {Fixture f;if(!f.Query()||!f.Prepare())return false;auto p=f.Permit();f.api.files[L"db/metadata"].bytes[12]=1;const auto source=f.api.files[L"db/reports/12345678-90ab-cdef-1122-334455667788.dmp"].bytes;return !f.Disposition(p)&&f.api.files[L"db/reports/12345678-90ab-cdef-1122-334455667788.dmp"].bytes==source&&!f.owner->reclaimed;}
bool EffectFailures() {
  Fixture baseline;if(!baseline.Query())return false;const auto start=baseline.api.effects;
  if(!baseline.Prepare())return false;const auto end=baseline.api.effects;
  for(uint32_t effect=start+1;effect<=end;++effect){Fixture f;if(!f.Query())return false;f.api.fail_effect=effect;if(f.Prepare()||f.owner->prepared||f.api.files[L"db/metadata"].bytes!=Metadata()||f.api.handles.empty())return false;}
  return true;
}
bool RecoverCase(unsigned mode) {
  Fixture f;
  if (!f.Query() || !f.Prepare()) return false;
  const auto permit=f.Permit(true);
  const auto candidate=Bytes(f.owner->candidate.begin(),f.owner->candidate.begin()+f.owner->candidate_bytes);
  const auto source_name=L"db/reports/12345678-90ab-cdef-1122-334455667788.dmp";
  if (mode != 0 && mode != 3) f.api.files[L"db/metadata"].bytes=candidate;
  if (mode == 2 || mode == 3 || mode == 4) f.api.files[source_name].present=false;
  if (mode == 4) {
    for (const auto& name:f.owner->stage_names) f.api.files[L"db/"+std::wstring(name.data())].present=false;
  }
  if (mode == 5) f.api.files[L"db/metadata"].inode=99;
  f.owner->in_cleanup=true;f.owner->cleanup={1000,1};
  if (!f.Retire() || !f.api.handles.empty()) return false;
  f.owner=std::make_unique<sf::Lease>(f.api);f.owner->operation={100,1};
  SsFinishedRecoverInputV1 input{};input.borrowed_database=1;input.borrowed_metrics=2;
  memcpy(input.expected_database_identity,permit.prepared.database_identity,24);memcpy(input.expected_metrics_identity,permit.prepared.metrics_identity,24);
  input.borrowed_live_recovery_permit=&permit;input.original_current_operation={100,1};
  const bool settled=f.Recover(input);
  if (mode == 3 || mode == 5) return !settled && !f.owner->reclaimed && !f.api.handles.empty();
  return settled && f.api.handles.empty() && f.owner->absent_mask==3 &&
      f.owner->reclaimed==((mode==2||mode==4)?0:permit.prepared.source.bytes) &&
      f.api.files[L"db/metadata"].bytes==candidate;
}
bool OriginalPresent(){return RecoverCase(0);}
bool CandidatePresent(){return RecoverCase(1);}
bool CandidateAbsent(){return RecoverCase(2);}
bool OriginalAbsent(){return RecoverCase(3);}
bool PriorAllAbsent(){return RecoverCase(4);}
bool ChangedMetadataInode(){return RecoverCase(5);}
bool DispositionFailures() {
  Fixture baseline;if(!baseline.Query()||!baseline.Prepare())return false;
  const auto start=baseline.api.effects;if(!baseline.Disposition(baseline.Permit()))return false;const auto end=baseline.api.effects;
  for(uint32_t effect=start+1;effect<=end;++effect){
    Fixture f;if(!f.Query()||!f.Prepare())return false;
    f.api.fail_effect=effect;
    if(f.Disposition(f.Permit())||f.owner->reclaimed||f.api.handles.empty())return false;
  }
  return true;
}
bool CancelAfterPartialOutputAndCleanup() {
  Fixture f;f.api.cancel_effect=1;
  if(f.Query()||f.api.effects!=1||f.api.handles.size()!=1||f.owner->primary!=ERROR_OPERATION_ABORTED)return false;
  const auto calls=f.main.calls;auto token=f.owner.release();
  SsFinishedCallV1 input{};input.abi_version=1;input.phase=Cleanup;input.lease=reinterpret_cast<uintptr_t>(token);input.original={1000,1};input.current=f.CleanupSource();
  SsFinishedResultV1 result{};ss_finished_retire_v1(&input,&result);
  if(!result.retired){f.owner.reset(token);return false;}
  return !result.lease&&!result.live_handles&&!result.live_locks&&result.primary_error==ERROR_OPERATION_ABORTED&&f.main.calls==calls&&f.cleanup.calls&&f.api.handles.empty();
}
bool CancelAtEachPreparationEffect() {
  Fixture baseline;if(!baseline.Query())return false;const auto start=baseline.api.effects;
  if(!baseline.Prepare())return false;const auto end=baseline.api.effects;
  for(uint32_t effect=start+1;effect<=end;++effect){Fixture f;if(!f.Query())return false;f.api.cancel_effect=effect;
    if(f.Prepare()||f.owner->prepared||f.api.effects!=effect||f.owner->primary!=ERROR_OPERATION_ABORTED||f.api.handles.empty()||f.api.files[L"db/metadata"].bytes!=Metadata())return false;
  }
  return true;
}
bool CancelAtEachDispositionEffect() {
  Fixture baseline;if(!baseline.Query()||!baseline.Prepare())return false;const auto start=baseline.api.effects;
  if(!baseline.Disposition(baseline.Permit()))return false;const auto end=baseline.api.effects;
  for(uint32_t effect=start+1;effect<=end;++effect){Fixture f;if(!f.Query()||!f.Prepare())return false;f.api.cancel_effect=effect;
    if(f.Disposition(f.Permit())||f.api.effects!=effect||f.owner->reclaimed||f.owner->primary!=ERROR_OPERATION_ABORTED)return false;
  }
  return true;
}
bool IncompleteInventory() { Fixture f;f.api.incomplete_scan=true;return f.Query()&&f.owner->facts_valid&&!f.owner->facts.inventory_complete; }
bool ForeignGeneration() { Fixture f;f.owner->expected.expected_database_identity[0]^=1;return !f.Query()&&f.owner->status==WrongIdentity&&!f.owner->facts_valid&&f.api.handles.size()==2; }
bool DistinctEpochStageNames() { Fixture f;if(!f.StageNames(f.Key()))return false;const auto first=f.owner->stage_names;auto key=f.Key();key.epoch[0]^=1;return f.StageNames(key)&&first!=f.owner->stage_names; }
bool QueryIsOneShot() { Fixture f; if (!f.Query()) return false; const auto effects=f.api.effects; const auto handles=f.api.handles.size(); return !f.Query()&&f.api.effects==effects&&f.api.handles.size()==handles; }
bool FirstCauseWinsPostTimeout() { Fixture f; f.api.fail_effect=f.api.late_effect=1; return !f.Query()&&f.owner->primary==9911&&f.owner->status==Failed&&f.owner->secondary_count&&f.owner->secondary[0]==ERROR_TIMEOUT&&f.api.handles.size()==1; }
bool HashVector() {
  Fixture fixture; sf::Invocation invocation(*fixture.owner, fixture.Source()); sf::HashState hash;const std::array<uint8_t,3> input{'a','b','c'};std::array<uint8_t,32> digest{};
  constexpr std::array<uint8_t,32> expected{0xba,0x78,0x16,0xbf,0x8f,0x01,0xcf,0xea,0x41,0x41,0x40,0xde,0x5d,0xae,0x22,0x23,0xb0,0x03,0x61,0xa3,0x96,0x17,0x7a,0x9c,0xb4,0x10,0xff,0x61,0xf2,0x00,0x15,0xad};
  const auto first=sf::BeginHash(hash, invocation),update=first?first:sf::UpdateHash(hash,input),finish=update?update:sf::FinishHash(hash,digest);
  const auto closed=sf::RetireHash(hash);return !first&&!update&&!finish&&!closed&&digest==expected&&!hash.hash&&!hash.algorithm;
}
struct RawControl { SsFinishedControlStatusV1 status; uint32_t cause; };
SsFinishedControlStatusV1 __cdecl ObserveRaw(uintptr_t state,uint32_t* cause) noexcept {
  const auto& value=*reinterpret_cast<const RawControl*>(state);*cause=value.cause;return value.status;
}
sf::Lease* Allocation(sf::FileApi& api) noexcept {
  auto& fake=static_cast<Fake&>(api);++fake.allocations;
  if(fake.fail_allocation){if(fake.stop_after_allocation)fake.failed_control=true;return nullptr;}
  auto owner=sf::NativeLeaseAllocation(api);
  if(fake.stop_after_allocation)fake.cancelled=true;
  return owner;
}
bool ClosedControlValidation() {
  const std::pair<RawControl,sf::Interruption> cases[]={
    {{FinishedControlCurrent,0},sf::Interruption::None},
    {{FinishedControlCancelled,ERROR_OPERATION_ABORTED},sf::Interruption::Cancelled},
    {{FinishedControlTimedOut,ERROR_TIMEOUT},sf::Interruption::DeadlineElapsed},
    {{FinishedControlFailed,ERROR_GEN_FAILURE},sf::Interruption::ClockFailure},
    {{FinishedControlCurrent,1},sf::Interruption::ClockFailure},
    {{FinishedControlCancelled,0},sf::Interruption::ClockFailure},
    {{FinishedControlTimedOut,0},sf::Interruption::ClockFailure},
    {{FinishedControlFailed,0},sf::Interruption::ClockFailure},
    {{static_cast<SsFinishedControlStatusV1>(4),0},sf::Interruption::ClockFailure}};
  for(const auto& [value,expected]:cases) {
    const SsFinishedControlSourceV1 source{reinterpret_cast<uintptr_t>(&value),ObserveRaw};
    if(sf::ObserveCurrent(source).interruption!=expected)return false;
  }
  return sf::ObserveCurrent({}).interruption==sf::Interruption::ClockFailure;
}
bool CancelledPreflightAllocatesNoLease() {
  RawControl control{FinishedControlCancelled,ERROR_OPERATION_ABORTED};
  LARGE_INTEGER now{},frequency{};
  if(!QueryPerformanceCounter(&now)||!QueryPerformanceFrequency(&frequency))return false;
  SsFinishedOpenInputV1 input{};input.abi_version=1;input.borrowed_database=1;input.borrowed_metrics=2;
  input.subject.tag=Desktop;input.subject.epoch[0]=1;input.subject.instance=1;input.subject.generation=1;
  input.build_length=1;input.build[0]='a';input.original_operation={now.QuadPart+frequency.QuadPart*10,frequency.QuadPart};
  input.current={reinterpret_cast<uintptr_t>(&control),ObserveRaw};
  SsFinishedResultV1 output{};ss_finished_open_v1(&input,&output);
  return output.primary_error==ERROR_OPERATION_ABORTED&&output.primary_origin==FinishedCauseControl&&output.status==Failed&&
      !output.lease&&!output.live_handles&&!output.live_locks&&output.retired==1&&!output.facts_valid&&!output.actual_read_bytes&&!output.actual_reclaimed_bytes;
}
bool PreflightPreservesAllocationCause() {
  RawControl control{FinishedControlCancelled,ERROR_OPERATION_ABORTED};LARGE_INTEGER now{},frequency{};
  if(!QueryPerformanceCounter(&now)||!QueryPerformanceFrequency(&frequency))return false;
  SsFinishedResultV1 output{};output.status=Failed;output.primary_error=ERROR_NOT_ENOUGH_MEMORY;
  output.primary_origin=FinishedCauseNative;
  const SsFinishedControlSourceV1 source{reinterpret_cast<uintptr_t>(&control),ObserveRaw};
  return !sf::Preflight(sf::WindowsFiles(),source,{now.QuadPart+frequency.QuadPart*10,frequency.QuadPart},output)&&
      output.primary_error==ERROR_NOT_ENOUGH_MEMORY&&output.cleanup_count==1&&output.cleanup_codes[0]==ERROR_OPERATION_ABORTED;
}
bool FailedPreflightCreatesNoOwner() {
  const std::pair<RawControl,SsFinishedCauseOriginV1> cases[]={
    {{FinishedControlCancelled,ERROR_OPERATION_ABORTED},FinishedCauseControl},
    {{FinishedControlFailed,ERROR_GEN_FAILURE},FinishedCauseControl},
    {{FinishedControlCurrent,0},FinishedCauseQpc}};
  for(const auto& [control,origin]:cases) {
    Fake api;if(origin==FinishedCauseQpc)api.now=100;
    const SsFinishedControlSourceV1 source{reinterpret_cast<uintptr_t>(&control),ObserveRaw};SsFinishedResultV1 result{};
    if(sf::CreateLease(api,source,{100,1},result,Allocation)||api.allocations||api.effects||!api.handles.empty()||
        result.retired!=1||result.lease||result.live_handles||result.live_locks||result.primary_origin!=origin)return false;
  }
  return true;
}
bool FailedAllocationProvesNoOwnerPreservingPostCause() {
  Fake api;api.fail_allocation=api.stop_after_allocation=true;TestControl control{api,false};
  const SsFinishedControlSourceV1 source{reinterpret_cast<uintptr_t>(&control),Current};SsFinishedResultV1 result{};
  return !sf::CreateLease(api,source,{100,1},result,Allocation)&&api.allocations==1&&!api.effects&&api.handles.empty()&&
      result.retired==1&&!result.lease&&!result.live_handles&&!result.live_locks&&!result.facts_valid&&
      result.primary_error==ERROR_NOT_ENOUGH_MEMORY&&result.primary_origin==FinishedCauseNative&&
      result.cleanup_count==1&&result.cleanup_codes[0]==ERROR_GEN_FAILURE&&result.cleanup_origin_bits==FinishedCauseControl;
}
bool CreatedZeroOutputOwnerRemainsUntilCheckedCleanup() {
  Fixture f;f.api.stop_after_allocation=true;SsFinishedResultV1 created{};
  auto owner=sf::CreateLease(f.api,f.Source(),{100,1},created,Allocation);
  if(!owner)return false;
  f.owner.reset(owner);
  if(created.retired||created.lease!=reinterpret_cast<uintptr_t>(owner)||created.live_handles||f.api.allocations!=1||
      f.Query()||f.api.effects)return false;
  auto token=f.owner.release();SsFinishedCallV1 call{};call.abi_version=1;call.phase=Cleanup;
  call.lease=reinterpret_cast<uintptr_t>(token);call.original={1000,1};call.current=f.CleanupSource();SsFinishedResultV1 retired{};
  ss_finished_retire_v1(&call,&retired);
  if(!retired.retired){f.owner.reset(token);return false;}
  return !retired.lease&&!retired.live_handles&&!retired.live_locks&&retired.primary_origin==FinishedCauseControl&&
      retired.primary_error==ERROR_OPERATION_ABORTED&&f.api.handles.empty();
}
bool EqualCodesPreserveActualNativeFirstOrigin() {
  Fixture f;f.api.fail_effect=f.api.failed_control_effect=1;f.api.failure_code=ERROR_GEN_FAILURE;
  if(f.Query())return false;SsFinishedResultV1 result{};f.owner->Result(result);
  return result.primary_error==ERROR_GEN_FAILURE&&result.primary_origin==FinishedCauseNative&&
      result.cleanup_count==1&&result.cleanup_codes[0]==ERROR_GEN_FAILURE&&result.cleanup_origin_bits==FinishedCauseControl&&f.api.effects==1;
}
bool ControlFirstNoEffectsAndNoNativeEcho() {
  Fixture f;RawControl failed{FinishedControlFailed,ERROR_GEN_FAILURE};
  sf::Invocation invocation(*f.owner,{reinterpret_cast<uintptr_t>(&failed),ObserveRaw});
  const auto error=invocation.Check();
  if(error.interruption==sf::Interruption::None||invocation.Post(error.cause)||f.api.effects)return false;
  SsFinishedResultV1 result{};f.owner->Result(result);
  return result.primary_origin==FinishedCauseControl&&result.primary_error==ERROR_GEN_FAILURE&&
      result.cleanup_count==1&&result.cleanup_codes[0]==ERROR_GEN_FAILURE&&result.cleanup_origin_bits==FinishedCauseControl;
}
bool QpcOriginAndBoundedSecondaryPacking() {
  Fixture f;f.api.late_effect=1;if(f.Query()||f.owner->primary_origin!=FinishedCauseQpc)return false;
  for(unsigned i=0;i<10;++i)f.owner->CleanupError(ERROR_GEN_FAILURE, i%2?FinishedCauseControl:FinishedCauseNative);
  SsFinishedResultV1 result{};f.owner->Result(result);
  if(result.cleanup_count!=8||result.omitted_cleanup!=2||(result.cleanup_origin_bits>>16))return false;
  for(unsigned i=0;i<8;++i)if(((result.cleanup_origin_bits>>(i*2))&3)!=(i%2?FinishedCauseControl:FinishedCauseNative))return false;
  return true;
}

}
int main() {
  const std::pair<const char*,bool(*)()> tests[]={{"normal",Normal},{"foreign-key",BoundsAndWrongKey},{"partial-open",PartialOutput},{"late-output",LatePartial},{"lock-busy",LockBusy},{"failed-close",FailedCloseRetains},{"collision",Collision},{"third-image",ThirdImage},{"every-prepare-effect",EffectFailures},{"original-present",OriginalPresent},{"candidate-present",CandidatePresent},{"candidate-absent",CandidateAbsent},{"original-absent",OriginalAbsent},{"prior-all-absent",PriorAllAbsent},{"changed-metadata-inode",ChangedMetadataInode},{"every-disposition-effect",DispositionFailures},{"CNG-sha256-vector",HashVector},{"first-cause-post-timeout",FirstCauseWinsPostTimeout},{"query-once",QueryIsOneShot},{"incomplete-inventory",IncompleteInventory},{"foreign-generation",ForeignGeneration},{"distinct-epoch-stage",DistinctEpochStageNames},{"cancel-partial-cleanup-source",CancelAfterPartialOutputAndCleanup},{"cancel-each-prepare",CancelAtEachPreparationEffect},{"cancel-each-disposition",CancelAtEachDispositionEffect}};
  for(const auto& [name,test]:tests)if(!test()){std::printf("FAILED %s\n",name);return 1;}
  if(!ClosedControlValidation()||!CancelledPreflightAllocatesNoLease()||!PreflightPreservesAllocationCause()){std::printf("FAILED control preflight\n");return 1;}
  if(!EqualCodesPreserveActualNativeFirstOrigin()||!ControlFirstNoEffectsAndNoNativeEcho()||!QpcOriginAndBoundedSecondaryPacking()){std::printf("FAILED cause origins\n");return 1;}
  if(!FailedPreflightCreatesNoOwner()||!FailedAllocationProvesNoOwnerPreservingPostCause()||!CreatedZeroOutputOwnerRemainsUntilCheckedCleanup()){std::printf("FAILED no-owner disposition\n");return 1;}
  std::printf("finished owner: %zu tests passed\n",std::size(tests)+9);return 0;
}
