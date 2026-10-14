// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "finished_io.h"
#include <winternl.h>
#include <aclapi.h>
#include <algorithm>
#include <cstring>
#include <limits>
namespace sugarsubstitute::finished {
namespace {
DWORD Last() noexcept { const auto e = GetLastError(); return e ? e : ERROR_GEN_FAILURE; }
DWORD Native(NTSTATUS status) noexcept {
  return status >= 0 ? ERROR_SUCCESS : RtlNtStatusToDosError(status);
}
bool Security(HANDLE file, std::span<uint32_t> buffer, DWORD& error, ParseControl& control) noexcept {
  const auto current = control.Check();
  if (current.interruption != Interruption::None) { error = current.cause; return false; }
  DWORD bytes = 0;
  if (!GetKernelObjectSecurity(file, OWNER_SECURITY_INFORMATION |
      DACL_SECURITY_INFORMATION | LABEL_SECURITY_INFORMATION,
      reinterpret_cast<PSECURITY_DESCRIPTOR>(buffer.data()),
      static_cast<DWORD>(buffer.size_bytes()), &bytes)) {
    error = Last(); return false;
  }
  if (bytes < SECURITY_DESCRIPTOR_MIN_LENGTH || bytes > buffer.size_bytes() ||
      !IsValidSecurityDescriptor(buffer.data())) { error = ERROR_INVALID_SECURITY_DESCR; return false; }
  return true;
}
DWORD Protection(HANDLE file, HANDLE parent, ParseControl& current) noexcept {
  std::array<uint32_t, 2048> actual{}, anchor{};
  DWORD error = 0;
  if (!Security(file, actual, error, current) || !Security(parent, anchor, error, current)) return error;
  PSID owner = nullptr, expected = nullptr; BOOL ignored = FALSE;
  if (!GetSecurityDescriptorOwner(actual.data(), &owner, &ignored) ||
      !GetSecurityDescriptorOwner(anchor.data(), &expected, &ignored)) return Last();
  if (!owner || !expected || !IsValidSid(owner) || !IsValidSid(expected) ||
      !EqualSid(owner, expected)) return ERROR_ACCESS_DENIED;
  PACL dacl = nullptr; BOOL present = FALSE;
  SECURITY_DESCRIPTOR_CONTROL control = 0; DWORD revision = 0;
  if (!GetSecurityDescriptorControl(actual.data(), &control, &revision) ||
      !GetSecurityDescriptorDacl(actual.data(), &present, &dacl, &ignored)) return Last();
  if (file == parent && !(control & SE_DACL_PROTECTED)) return ERROR_ACCESS_DENIED;
  std::array<uint8_t, SECURITY_MAX_SID_SIZE> system{}; DWORD size = static_cast<DWORD>(system.size());
  if (!CreateWellKnownSid(WinLocalSystemSid, nullptr, system.data(), &size)) return Last();
  const auto restricted_acl = [&](PSECURITY_DESCRIPTOR descriptor) noexcept -> DWORD {
    PACL acl = nullptr; BOOL has_acl = FALSE, defaulted = FALSE;
    if (!GetSecurityDescriptorDacl(descriptor, &has_acl, &acl, &defaulted)) return Last();
    if (!has_acl || !acl || !IsValidAcl(acl) || acl->AceCount != 2) return ERROR_ACCESS_DENIED;
    for (DWORD i = 0; i < 2; ++i) {
      void* raw = nullptr;
      if (!GetAce(acl, i, &raw)) return Last();
      const auto ace = static_cast<ACCESS_ALLOWED_ACE*>(raw);
      if (ace->Header.AceType != ACCESS_ALLOWED_ACE_TYPE || ace->Mask != FILE_ALL_ACCESS ||
          !IsValidSid(&ace->SidStart) || !EqualSid(&ace->SidStart, i == 0 ? expected : system.data())) return ERROR_ACCESS_DENIED;
    }
    return ERROR_SUCCESS;
  };
  error = restricted_acl(actual.data());
  if (error) return error;
  error = restricted_acl(anchor.data());
  if (error) return error;
  // Inherited file ACEs retain only the account/SYSTEM rights from the pinned
  // protected parent. No untrusted grant or reparse traversal is accepted.
  PACL sacl = nullptr;
  if (!GetSecurityDescriptorSacl(actual.data(), &present, &sacl, &ignored)) return Last();
  // Windows MIC assigns medium/no-write-up to an unlabeled object. Pinned
  // Crashpad creates report/metadata children with inherited account ACLs and
  // no explicit label. The retained protected root itself must stay explicit.
  if ((!present || !sacl || (IsValidAcl(sacl) && !sacl->AceCount)) && file != parent)
    return ERROR_SUCCESS;
  if (!present || !sacl || !IsValidAcl(sacl) || sacl->AceCount != 1) return ERROR_ACCESS_DENIED;
  void* label = nullptr;
  if (!GetAce(sacl, 0, &label)) return Last();
  const auto ace = static_cast<SYSTEM_MANDATORY_LABEL_ACE*>(label);
  std::array<uint8_t, SECURITY_MAX_SID_SIZE> medium{}; size = static_cast<DWORD>(medium.size());
  if (!CreateWellKnownSid(WinMediumLabelSid, nullptr, medium.data(), &size)) return Last();
  return ace->Header.AceType == SYSTEM_MANDATORY_LABEL_ACE_TYPE &&
      ace->Mask == SYSTEM_MANDATORY_LABEL_NO_WRITE_UP &&
      IsValidSid(&ace->SidStart) && EqualSid(&ace->SidStart, medium.data())
      ? ERROR_SUCCESS : ERROR_ACCESS_DENIED;
}
class WindowsApi final : public FileApi {
 public:
  DWORD Clock(int64_t& counter, int64_t& frequency) noexcept override {
    LARGE_INTEGER ticks{}, hz{};
    if (!QueryPerformanceCounter(&ticks) || !QueryPerformanceFrequency(&hz)) return Last();
    counter = ticks.QuadPart; frequency = hz.QuadPart; return 0;
  }
  DWORD Duplicate(HANDLE source, HANDLE& output) noexcept override {
    return DuplicateHandle(GetCurrentProcess(), source, GetCurrentProcess(),
        &output, 0, FALSE, DUPLICATE_SAME_ACCESS) ? 0 : Last();
  }
  DWORD Open(HANDLE parent, const wchar_t* leaf, OpenMode mode, HANDLE& output, ParseControl& control) noexcept override {
    const size_t count = wcsnlen(leaf, 80);
    if (!parent || count == 0 || count == 80) return ERROR_INVALID_PARAMETER;
    for (size_t i = 0; i < count; ++i)
      if (leaf[i] == L'/' || leaf[i] == L'\\' || leaf[i] == L':') return ERROR_INVALID_PARAMETER;
    UNICODE_STRING name{static_cast<USHORT>(count * 2), static_cast<USHORT>(count * 2), const_cast<PWSTR>(leaf)};
    OBJECT_ATTRIBUTES attributes{};
    InitializeObjectAttributes(&attributes, &name, OBJ_CASE_INSENSITIVE | 0x1000, parent, nullptr);
    std::array<uint32_t, 2048> descriptor{};
    if (mode == OpenMode::CreateStage) {
      DWORD error = 0;
      if (!Security(parent, descriptor, error, control)) return error;
      attributes.SecurityDescriptor = descriptor.data();
    }
    const bool directory = mode == OpenMode::Directory;
    ACCESS_MASK access = READ_CONTROL | SYNCHRONIZE | (directory ? FILE_LIST_DIRECTORY : FILE_READ_DATA);
    if (!directory) access |= FILE_READ_ATTRIBUTES;
    if (mode == OpenMode::Metadata || mode == OpenMode::CreateStage) access |= FILE_WRITE_DATA;
    if (mode == OpenMode::Read || mode == OpenMode::CreateStage || mode == OpenMode::RecoverStage) access |= DELETE;
    ULONG share = FILE_SHARE_READ;
    if (mode == OpenMode::Metadata || directory) share |= FILE_SHARE_WRITE;
    IO_STATUS_BLOCK io{};
    // Synchronous relative FILE_OPEN/FILE_CREATE: no ambient path, no follows.
    const auto before = control.Check();
    if (before.interruption != Interruption::None) return before.cause;
    return Native(NtCreateFile(&output, access, &attributes, &io, nullptr,
        FILE_ATTRIBUTE_NORMAL, share, mode == OpenMode::CreateStage ? 2 : 1,
        0x20 | 0x200000 | (directory ? 1 : 0x40), nullptr, 0));
  }
  DWORD Inspect(HANDLE file, HANDLE parent, FileInfo& output, ParseControl& control) noexcept override {
    FILE_ATTRIBUTE_TAG_INFO tag{}; FILE_ID_INFO identity{}; LARGE_INTEGER size{};
    auto current = control.Check();
    if (current.interruption != Interruption::None) return current.cause;
    if (!GetFileInformationByHandleEx(file, FileAttributeTagInfo, &tag, sizeof(tag))) return Last();
    current = control.Check();
    if (current.interruption != Interruption::None) return current.cause;
    if (!GetFileInformationByHandleEx(file, FileIdInfo, &identity, sizeof(identity))) return Last();
    if (tag.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) return ERROR_REPARSE_TAG_INVALID;
    output.directory = !!(tag.FileAttributes & FILE_ATTRIBUTE_DIRECTORY);
    if (!output.directory) {
      current = control.Check();
      if (current.interruption != Interruption::None) return current.cause;
      if (!GetFileSizeEx(file, &size)) return Last();
      if (size.QuadPart < 0) return ERROR_INVALID_DATA;
    }
    output.bytes = static_cast<uint64_t>(size.QuadPart);
    static_assert(sizeof(identity) == 24);
    memcpy(output.identity.data(), &identity, 24);
    return Protection(file, parent, control);
  }
  DWORD Read(HANDLE file, uint64_t offset, std::span<uint8_t> output, ParseControl& control) noexcept override {
    LARGE_INTEGER position{}; position.QuadPart = static_cast<LONGLONG>(offset);
    if (offset > static_cast<uint64_t>(INT64_MAX) || output.size() > 65536) return ERROR_INVALID_PARAMETER;
    if (!SetFilePointerEx(file, position, nullptr, FILE_BEGIN)) return Last();
    DWORD bytes = 0;
    const auto before = control.Check();
    if (before.interruption != Interruption::None) return before.cause;
    if (!ReadFile(file, output.data(), static_cast<DWORD>(output.size()), &bytes, nullptr)) return Last();
    return bytes == output.size() ? 0 : ERROR_HANDLE_EOF;
  }
  DWORD Write(HANDLE file, uint64_t offset, std::span<const uint8_t> image, ParseControl& control) noexcept override {
    if (offset > static_cast<uint64_t>(INT64_MAX) || image.size() > 65536) return ERROR_INVALID_PARAMETER;
    LARGE_INTEGER position{}; position.QuadPart = static_cast<LONGLONG>(offset);
    if (!SetFilePointerEx(file, position, nullptr, FILE_BEGIN)) return Last();
    DWORD written = 0;
    const auto before = control.Check();
    if (before.interruption != Interruption::None) return before.cause;
    if (!WriteFile(file, image.data(), static_cast<DWORD>(image.size()), &written, nullptr)) return Last();
    return written == image.size() ? 0 : ERROR_WRITE_FAULT;
  }
  DWORD Truncate(HANDLE file, uint64_t bytes, ParseControl& control) noexcept override {
    LARGE_INTEGER position{}; position.QuadPart = static_cast<LONGLONG>(bytes);
    if (bytes > static_cast<uint64_t>(INT64_MAX)) return ERROR_INVALID_PARAMETER;
    if (!SetFilePointerEx(file, position, nullptr, FILE_BEGIN)) return Last();
    const auto before = control.Check();
    if (before.interruption != Interruption::None) return before.cause;
    return SetEndOfFile(file) ? 0 : Last();
  }
  DWORD Flush(HANDLE file) noexcept override { return FlushFileBuffers(file) ? 0 : Last(); }
  DWORD Lock(HANDLE file, bool exclusive) noexcept override {
    OVERLAPPED overlapped{};
    return LockFileEx(file, LOCKFILE_FAIL_IMMEDIATELY | (exclusive ? LOCKFILE_EXCLUSIVE_LOCK : 0),
        0, MAXDWORD, MAXDWORD, &overlapped) ? 0 : Last();
  }
  DWORD Unlock(HANDLE file) noexcept override {
    OVERLAPPED overlapped{};
    return UnlockFileEx(file, 0, MAXDWORD, MAXDWORD, &overlapped) ? 0 : Last();
  }
  DWORD Dispose(HANDLE file) noexcept override {
    FILE_DISPOSITION_INFO information{TRUE};
    return SetFileInformationByHandle(file, FileDispositionInfo, &information, sizeof(information)) ? 0 : Last();
  }
  DWORD Close(HANDLE file) noexcept override { return CloseHandle(file) ? 0 : Last(); }
  DWORD Scan(HANDLE directory, ScanRole role, bool restart, std::span<uint8_t> page,
      uint32_t& entries, uint64_t& bytes, bool& end, bool& complete) noexcept override {
    entries = 0; bytes = 0; end = false; complete = true;
    if (!GetFileInformationByHandleEx(directory, restart ? FileIdBothDirectoryRestartInfo : FileIdBothDirectoryInfo,
        page.data(), static_cast<DWORD>(page.size()))) {
      const auto error = Last(); if (error == ERROR_NO_MORE_FILES) { end = true; return 0; } return error;
    }
    size_t offset = 0;
    for (;;) {
      constexpr size_t header = offsetof(FILE_ID_BOTH_DIR_INFO, FileName);
      if (offset % 8 || offset + header > page.size()) return ERROR_INVALID_DATA;
      const auto info = reinterpret_cast<const FILE_ID_BOTH_DIR_INFO*>(page.data() + offset);
      if (info->FileNameLength % 2 || info->FileNameLength > page.size() - offset - header ||
          info->EndOfFile.QuadPart < 0) return ERROR_INVALID_DATA;
      const auto units = info->FileNameLength / 2;
      const bool dot = (units == 1 && info->FileName[0] == L'.') ||
          (units == 2 && info->FileName[0] == L'.' && info->FileName[1] == L'.');
      if (!dot) {
        if (info->FileAttributes & FILE_ATTRIBUTE_DIRECTORY) {
          const bool reports = role == ScanRole::Database && units == 7 &&
              std::equal(info->FileName, info->FileName + 7, L"reports");
          if (!reports) complete = false;
        }
        ++entries;
        if (!(info->FileAttributes & FILE_ATTRIBUTE_DIRECTORY)) {
          const auto size = static_cast<uint64_t>(info->EndOfFile.QuadPart);
          if (size > UINT64_MAX - bytes) return ERROR_ARITHMETIC_OVERFLOW;
          bytes += size;
        }
        if (info->FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) return ERROR_REPARSE_TAG_INVALID;
      }
      const auto next = info->NextEntryOffset;
      if (!next) return 0;
      if (next % 8 || next < header + info->FileNameLength || next > page.size() - offset - header)
        return ERROR_INVALID_DATA;
      offset += next;
    }
  }
  DWORD HashBegin(HashState& state, ParseControl& control) noexcept override { return BeginHash(state, control); }
  DWORD HashUpdate(HashState& state, std::span<const uint8_t> bytes) noexcept override { return UpdateHash(state, bytes); }
  DWORD HashFinish(HashState& state, std::span<uint8_t, 32> digest) noexcept override { return FinishHash(state, digest); }
  DWORD HashRetire(HashState& state, bool hash) noexcept override { return RetireHashPart(state, hash); }
};
WindowsApi native;
}  // namespace
FileApi& WindowsFiles() noexcept { return native; }
}  // namespace sugarsubstitute::finished
