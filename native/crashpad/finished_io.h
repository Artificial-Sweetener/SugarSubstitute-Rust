// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#ifndef SUGARSUBSTITUTE_FINISHED_IO_H_
#define SUGARSUBSTITUTE_FINISHED_IO_H_
#include "finished_report_abi.h"
#include "bounded_metadata.h"
#include <windows.h>
#include "finished_digest.h"
#include <array>
#include <span>

namespace sugarsubstitute::finished {
struct FileInfo {
  std::array<uint8_t, 24> identity{};
  uint64_t bytes = 0;
  bool directory = false;
};
enum class ScanRole { Database, Reports, Metrics, Attachments };
enum class OpenMode { Directory, Read, Metadata, CreateStage, RecoverStage };
// Only OS calls are replaced by tests. All output slots belong to the caller
// before their status is inspected, including partial outputs on error.
class FileApi {
 public:
  virtual ~FileApi() = default;
  virtual DWORD Clock(int64_t& counter, int64_t& frequency) noexcept = 0;
  virtual DWORD Duplicate(HANDLE source, HANDLE& output) noexcept = 0;
  virtual DWORD Open(HANDLE parent, const wchar_t* leaf, OpenMode mode,
                     HANDLE& output, ParseControl& control) noexcept = 0;
  virtual DWORD Inspect(HANDLE file, HANDLE protected_parent,
                        FileInfo& output, ParseControl& control) noexcept = 0;
  virtual DWORD Read(HANDLE file, uint64_t offset,
                     std::span<uint8_t> output, ParseControl& control) noexcept = 0;
  virtual DWORD Write(HANDLE file, uint64_t offset, std::span<const uint8_t> image, ParseControl& control) noexcept = 0;
  virtual DWORD Truncate(HANDLE file, uint64_t bytes, ParseControl& control) noexcept = 0;
  virtual DWORD Flush(HANDLE file) noexcept = 0;
  virtual DWORD Lock(HANDLE file, bool exclusive) noexcept = 0;
  virtual DWORD Unlock(HANDLE file) noexcept = 0;
  virtual DWORD Dispose(HANDLE file) noexcept = 0;
  virtual DWORD Close(HANDLE file) noexcept = 0;
  virtual DWORD Scan(HANDLE directory, ScanRole role, bool restart, std::span<uint8_t> page,
                     uint32_t& entries, uint64_t& bytes, bool& end, bool& complete) noexcept = 0;
  virtual DWORD HashBegin(HashState& state, ParseControl& control) noexcept = 0;
  virtual DWORD HashUpdate(HashState& state,
                           std::span<const uint8_t> bytes) noexcept = 0;
  virtual DWORD HashFinish(HashState& state,
                           std::span<uint8_t, 32> digest) noexcept = 0;
  virtual DWORD HashRetire(HashState& state, bool hash) noexcept = 0;
};
FileApi& WindowsFiles() noexcept;
}  // namespace sugarsubstitute::finished
#endif
