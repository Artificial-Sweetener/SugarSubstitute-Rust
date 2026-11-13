// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#ifndef SUGARSUBSTITUTE_INCIDENT_BRIDGE_H_
#define SUGARSUBSTITUTE_INCIDENT_BRIDGE_H_

#include "bounded_metadata.h"

namespace sugarsubstitute::finished {

inline constexpr std::uint32_t kDirectoryLimit = 128;
inline constexpr std::uint32_t kModuleLimit = 1024;
inline constexpr std::uint32_t kDictionaryEntryLimit = 32;
inline constexpr std::uint32_t kOwnedAnnotationLimit = 32;
inline constexpr std::uint32_t kKeyLimit = 64;
inline constexpr std::uint32_t kValueLimit = 256;

struct ReadResult {
  bool complete = false;
  std::uint32_t cause = 0;
};
// The caller retains the actual immutable protected file and its size. Each
// read must either fill the requested bounded range or preserve its original
// failure. No ownership, path, native handle, timer or retry escapes this
// external port.
class DumpSource {
 public:
  virtual ~DumpSource() = default;
  virtual ReadResult Read(std::uint64_t offset,
                          std::span<std::uint8_t> output) noexcept = 0;
};

enum class SubjectRole { Supervisor, Desktop };
struct ExpectedIdentity {
  std::array<std::uint8_t, 32> name_space{};
  std::array<std::uint8_t, 16> epoch{};
  SubjectRole role = SubjectRole::Supervisor;
  std::uint64_t instance = 0;
  std::uint64_t generation = 0;
  std::array<std::uint8_t, 64> build{};
  std::uint32_t build_length = 0;
  Uuid report_uuid{};
};
struct DumpIdentityFacts {
  Uuid report_uuid{};
  std::uint32_t module_index = 0;
  std::uint32_t dictionaries = 0;
  std::uint32_t entries = 0;
};

// Read pinned version-one Crashpad identity dictionaries using fixed storage
// and checked file-relative ranges. Exactly one module must contain the
// complete product/build/role/coordinator/subject tuple. Process annotations
// may repeat matching facts but never complete a partial module tuple. The
// canonical UUID must match the selected metadata UUID, including any upstream
// exact filter. ExpectedIdentity is inert matching data, not a file or native
// authority. The external control supplies the original
// deadline/cancellation/clock cause; every source read checks it before and
// after. No new deadline is created. The caller holds source/control/input
// live, on their required thread, throughout this synchronous call. On every
// failure, output is cleared and cause preserved.
ParseResult ParseCrashpadIdentity(DumpSource& source, std::uint64_t file_size,
                                  const ExpectedIdentity& expected,
                                  ParseControl& control,
                                  DumpIdentityFacts& output) noexcept;

}  // namespace sugarsubstitute::finished
#endif  // SUGARSUBSTITUTE_INCIDENT_BRIDGE_H_
