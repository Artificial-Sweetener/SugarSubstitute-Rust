// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "finished_report.h"
#include <cstring>

namespace sugarsubstitute::finished {
namespace {
void UuidName(const Uuid& uuid, std::span<wchar_t, 41> name) noexcept {
  constexpr wchar_t hex[] = L"0123456789abcdef";
  size_t position = 0;
  for (size_t i = 0; i < uuid.size(); ++i) {
    if (i == 4 || i == 6 || i == 8 || i == 10) name[position++] = L'-';
    name[position++] = hex[uuid[i] >> 4]; name[position++] = hex[uuid[i] & 15];
  }
  std::copy_n(L".dmp", 5, name.data() + 36);
}
ExpectedIdentity Expected(const QueryIdentity& input, const Uuid& uuid) noexcept {
  ExpectedIdentity expected{};
  memcpy(expected.name_space.data(), input.subject.namespace_id, 32);
  memcpy(expected.epoch.data(), input.subject.epoch, 16);
  expected.role = input.subject.tag == Desktop ? SubjectRole::Desktop : SubjectRole::Supervisor;
  expected.instance = input.subject.instance; expected.generation = input.subject.generation;
  memcpy(expected.build.data(), input.build, 64); expected.build_length = input.build_length;
  expected.report_uuid = uuid; return expected;
}
bool InventoryPages(Invocation& invocation, ScanRole role) noexcept {
  auto& owner = invocation.state;
  bool restart = true;
  for (;;) {
    if (!invocation.Allowed()) return false;
    uint32_t entries = 0; uint64_t bytes = 0; bool end = false, complete = false;
    if (!invocation.Post(owner.api.Scan(owner.handles[ScanDirectory], role, restart, owner.scratch, entries, bytes, end, complete))) return false;
    restart = false;
    if (!complete) owner.facts.inventory_complete = 0;
    if (entries > 4096 - owner.facts.observed_entries ||
        bytes > UINT64_MAX - owner.facts.observed_temporary_bytes) {
      owner.facts.inventory_complete = 0;
      return role == ScanRole::Attachments ? owner.Fail(ERROR_NOT_SUPPORTED) : invocation.Close(ScanDirectory);
    }
    owner.facts.observed_entries += entries; owner.facts.observed_temporary_bytes += bytes;
    if (role == ScanRole::Attachments && (entries || bytes || !complete)) {
      owner.facts.inventory_complete = 0;
      return owner.Fail(ERROR_NOT_SUPPORTED);
    }
    if (end) return invocation.Close(ScanDirectory);
  }
}
bool Inventory(Invocation& invocation, Slot parent, ScanRole role) noexcept {
  return invocation.Open(ScanDirectory, parent, L".", OpenMode::Directory) && InventoryPages(invocation, role);
}
}  // namespace
bool InventoryAttachments(Invocation& invocation) noexcept {
  if (!invocation.Open(ScanDirectory, Database, L"attachments", OpenMode::Directory)) return false;
  FileInfo info{};
  if (!invocation.Info(ScanDirectory, Database, info)) return false;
  if (!info.directory) return invocation.state.Fail(ERROR_INVALID_DATA);
  return InventoryPages(invocation, ScanRole::Attachments);
}
bool Query(Invocation& invocation) noexcept {
  auto& owner = invocation.state;
  if (owner.queried) return owner.Fail(ERROR_INVALID_STATE);
  owner.queried = true;
  const auto& input = owner.expected;
  if (!invocation.Allowed() || !invocation.Post(owner.api.Duplicate(reinterpret_cast<HANDLE>(input.borrowed_database), owner.handles[Database])) ||
      !invocation.Allowed() || !invocation.Post(owner.api.Duplicate(reinterpret_cast<HANDLE>(input.borrowed_metrics), owner.handles[Metrics]))) return false;
  FileInfo database{}, metrics{};
  if (!invocation.Info(Database, Database, database) || !invocation.Info(Metrics, Metrics, metrics)) return false;
  if (!database.directory || !metrics.directory ||
      memcmp(database.identity.data(), input.expected_database_identity, 24) ||
      memcmp(metrics.identity.data(), input.expected_metrics_identity, 24)) { owner.status = WrongIdentity; return false; }
  if (!invocation.Open(Reports, Database, L"reports", OpenMode::Directory) ||
      !invocation.Open(Metadata, Database, L"metadata", OpenMode::Metadata)) return false;
  FileInfo reports{};
  if (!invocation.Info(Reports, Database, reports) || !reports.directory) return owner.Fail(ERROR_INVALID_DATA);
  if (!invocation.LockMetadata(false)) return false;
  FileInfo metadata{};
  if (!invocation.Info(Metadata, Database, metadata)) return false;
  if (metadata.directory || metadata.bytes > kMetadataLimit) { owner.status = Oversized; return false; }
  owner.metadata_bytes = static_cast<uint32_t>(metadata.bytes);
  if (!invocation.Image(Metadata, owner.metadata, metadata.bytes)) return false;
  MetadataFacts parsed{};
  const auto parse = ParseCrashpadMetadata(std::span(owner.metadata).first(owner.metadata_bytes), invocation, parsed);
  if (parse.status != ParseStatus::Valid) {
    owner.status = Status(parse.status); return false;
  }
  bool selected = false;
  for (uint32_t i = 0; i < parsed.count; ++i) {
    const auto& record = parsed.records[i];
    if (input.has_uuid && memcmp(record.uuid.data(), input.optional_uuid, 16)) continue;
    if (record.state == ReportState::Uploading) { owner.status = Busy; return false; }
    std::array<wchar_t, 41> name{}; UuidName(record.uuid, name);
    if (!invocation.Open(Candidate, Reports, name.data(), OpenMode::Read)) return false;
    FileInfo info{};
    if (!invocation.Info(Candidate, Reports, info)) return false;
    if (info.directory || info.bytes > kDumpLimit) { owner.status = Oversized; return false; }
    owner.read_slot = Candidate; owner.read_size = info.bytes;
    DumpIdentityFacts identity{};
    const auto identity_result = ParseCrashpadIdentity(invocation, info.bytes, Expected(input, record.uuid), invocation, identity);
    if (identity_result.status == ParseStatus::WrongIdentity) {
      if (!invocation.Close(Candidate)) return false;
      continue;
    }
    if (identity_result.status != ParseStatus::Valid) {
      owner.status = Status(identity_result.status); return false;
    }
    if (selected) { owner.status = WrongIdentity; return false; }
    // Retain the actual selected inode, not a filename-derived capability.
    owner.handles[Source] = owner.handles[Candidate]; owner.handles[Candidate] = nullptr;
    owner.read_slot = Source; owner.read_size = info.bytes; selected = true;
    owner.source_name = name;
    owner.facts.subject = input.subject; memcpy(owner.facts.uuid, record.uuid.data(), 16);
    owner.facts.build_length = input.build_length; memcpy(owner.facts.build, input.build, 64);
    owner.facts.source_bytes = info.bytes; memcpy(owner.facts.source_file_identity, info.identity.data(), 24);
  }
  if (!selected) { owner.status = Missing; return false; }
  if (!invocation.UnlockMetadata()) return false;
  owner.facts.inventory_complete = 1;
  if (!InventoryAttachments(invocation) || !Inventory(invocation, Database, ScanRole::Database) ||
      !Inventory(invocation, Reports, ScanRole::Reports) || !Inventory(invocation, Metrics, ScanRole::Metrics)) return false;
  owner.facts_valid = true; owner.status = Finished; return true;
}
}  // namespace sugarsubstitute::finished
