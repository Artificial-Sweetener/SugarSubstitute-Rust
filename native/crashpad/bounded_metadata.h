// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#ifndef SUGARSUBSTITUTE_BOUNDED_METADATA_H_
#define SUGARSUBSTITUTE_BOUNDED_METADATA_H_

#include <array>
#include <cstddef>
#include <cstdint>
#include <span>

namespace sugarsubstitute::finished {

// Pinned Crashpad Windows metadata v1: header followed by native 56-byte records.
inline constexpr std::uint32_t kMetadataHeaderBytes = 16;
inline constexpr std::uint32_t kMetadataRecordBytes = 56;
inline constexpr std::uint64_t kMetadataLimit = 1024 * 1024;
inline constexpr std::uint32_t kReportLimit = 256;
inline constexpr std::uint64_t kDumpLimit = 256 * 1024 * 1024;
using Uuid = std::array<std::uint8_t, 16>;

// These are parsing observations. They grant no native handle or disposition
// right.
enum class ParseStatus {
  Valid,
  Missing,
  Busy,
  Corrupt,
  WrongIdentity,
  Oversized,
  DeadlineElapsed,
  Cancelled,
  ClockFailure,
  SourceFailure,
};
struct ParseResult {
  ParseStatus status = ParseStatus::Valid;
  std::uint64_t offset = 0;
  std::uint32_t cause = 0;
};
enum class Interruption { None, DeadlineElapsed, Cancelled, ClockFailure };
struct Checkpoint {
  Interruption interruption = Interruption::None;
  std::uint32_t cause = 0;
};
// The admitted native caller supplies its original execution/QPC binding. The
// parser creates no clock, deadline, timer, wait or retry and retains no
// callback.
class ParseControl {
 public:
  virtual ~ParseControl() = default;
  virtual Checkpoint Check() noexcept = 0;
};
enum class ReportState : std::uint32_t {
  Pending = 0,
  Uploading = 1,
  Completed = 2
};
struct MetadataRecord {
  Uuid uuid{};
  ReportState state = ReportState::Pending;
  std::uint32_t offset = 0;
};
struct MetadataFacts {
  std::array<MetadataRecord, kReportLimit> records{};
  std::uint32_t count = 0;
  std::uint32_t string_table_offset = 0;
};

// Parse the pinned Crashpad version-one Windows metadata layout without
// allocation or path construction. `output` is valid only on Valid; all other
// results clear it. The immutable caller-owned image must remain live for the
// call. All strings, states, UUIDs and references are checked, including
// records outside selection.
ParseResult ParseCrashpadMetadata(std::span<const std::uint8_t> image,
                                  ParseControl& control,
                                  MetadataFacts& output) noexcept;
// Find an exact nonzero canonical UUID in a previously validated immutable
// view. Uploading is Busy. No ordering, timestamp or path spelling selects a
// report.
ParseResult FindMetadataUuid(const MetadataFacts& facts, const Uuid& uuid,
                             std::uint32_t& record_index) noexcept;
// Convert native UUID integer fields explicitly to canonical RFC byte order.
Uuid CanonicalUuid(std::span<const std::uint8_t, 16> native) noexcept;
// Validate bounded UTF-8 without allocating, accepting Unicode but no malformed
// continuation, overlong sequence, surrogate or value outside Unicode.
bool ValidUtf8(std::span<const std::uint8_t> text) noexcept;
// Propagate the external owner's original interruption and its exact cause.
ParseResult CheckProgress(ParseControl& control, std::uint64_t offset) noexcept;

}  // namespace sugarsubstitute::finished
#endif  // SUGARSUBSTITUTE_BOUNDED_METADATA_H_
