// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "bounded_metadata.h"

#include <algorithm>

namespace sugarsubstitute::finished {
namespace {
constexpr std::uint32_t kMagic = 0x43504144;

std::uint32_t U32(std::span<const std::uint8_t> image,
                  std::size_t offset) noexcept {
  std::uint32_t value = 0;
  for (std::size_t i = 0; i < 4; ++i) {
    value |= static_cast<std::uint32_t>(image[offset + i]) << (8 * i);
  }
  return value;
}
bool Nonzero(const Uuid& uuid) noexcept {
  return std::any_of(uuid.begin(), uuid.end(),
                     [](auto byte) { return byte != 0; });
}
bool StringAt(std::span<const std::uint8_t> table, std::uint32_t index,
              bool filename, const Uuid& uuid) noexcept {
  if (index >= table.size() || (index != 0 && table[index - 1] != 0)) {
    return false;
  }
  if (!filename) {
    return true;
  }
  std::size_t end = index;
  const auto limit = std::min(table.size(), end + 41);
  while (end < limit && table[end] != 0) {
    ++end;
  }
  if (end == limit) {
    return false;
  }
  const auto text = table.subspan(index, end - index);
  if (text.size() != 40 || text[36] != '.' || text[37] != 'd' ||
      text[38] != 'm' || text[39] != 'p') {
    return false;
  }
  constexpr char hex[] = "0123456789abcdef";
  std::size_t position = 0;
  for (std::size_t i = 0; i < uuid.size(); ++i) {
    if (i == 4 || i == 6 || i == 8 || i == 10) {
      if (text[position++] != '-') {
        return false;
      }
    }
    if (text[position++] != hex[uuid[i] >> 4] ||
        text[position++] != hex[uuid[i] & 15]) {
      return false;
    }
  }
  return true;
}
ParseResult Parse(std::span<const std::uint8_t> image, ParseControl& control,
                  MetadataFacts& facts) noexcept {
  if (image.size() > kMetadataLimit) {
    return {ParseStatus::Oversized};
  }
  if (image.size() < kMetadataHeaderBytes || U32(image, 0) != kMagic ||
      U32(image, 4) != 1 || U32(image, 12) != 0) {
    return {ParseStatus::Corrupt};
  }
  facts.count = U32(image, 8);
  if (facts.count > kReportLimit) {
    return {ParseStatus::Oversized, 8};
  }
  const std::uint64_t table_offset =
      kMetadataHeaderBytes + static_cast<std::uint64_t>(facts.count) * kMetadataRecordBytes;
  if (table_offset > image.size()) {
    return {ParseStatus::Corrupt, table_offset};
  }
  facts.string_table_offset = static_cast<std::uint32_t>(table_offset);
  const auto table = image.subspan(static_cast<std::size_t>(table_offset));
  if (!table.empty() && (table.back() != 0 || !ValidUtf8(table))) {
    return {ParseStatus::Corrupt, table_offset};
  }
  for (std::uint32_t i = 0; i < facts.count; ++i) {
    const std::size_t offset =
        kMetadataHeaderBytes + static_cast<std::size_t>(i) * kMetadataRecordBytes;
    const auto progress = CheckProgress(control, offset);
    if (progress.status != ParseStatus::Valid) {
      return progress;
    }
    auto& record = facts.records[i];
    record.offset = static_cast<std::uint32_t>(offset);
    record.uuid = CanonicalUuid(
        std::span<const std::uint8_t, 16>(image.data() + offset, 16));
    if (!Nonzero(record.uuid)) {
      return {ParseStatus::WrongIdentity, offset};
    }
    for (std::uint32_t previous = 0; previous < i; ++previous) {
      if (facts.records[previous].uuid == record.uuid) {
        return {ParseStatus::WrongIdentity, offset};
      }
    }
    const auto state = U32(image, offset + 44);
    if (state > static_cast<std::uint32_t>(ReportState::Completed) ||
        (image[offset + 48] & ~3u) != 0 ||
        !std::all_of(image.begin() + offset + 49, image.begin() + offset + kMetadataRecordBytes,
                     [](auto byte) { return byte == 0; })) {
      return {ParseStatus::Corrupt, offset + 44};
    }
    record.state = static_cast<ReportState>(state);
    if (!StringAt(table, U32(image, offset + 16), true, record.uuid) ||
        !StringAt(table, U32(image, offset + 20), false, record.uuid)) {
      return {ParseStatus::Corrupt, offset + 16};
    }
  }
  return CheckProgress(control, image.size());
}
}  // namespace

ParseResult CheckProgress(ParseControl& control,
                          std::uint64_t offset) noexcept {
  const auto check = control.Check();
  switch (check.interruption) {
    case Interruption::None:
      return {};
    case Interruption::DeadlineElapsed:
      return {ParseStatus::DeadlineElapsed, offset, check.cause};
    case Interruption::Cancelled:
      return {ParseStatus::Cancelled, offset, check.cause};
    case Interruption::ClockFailure:
      return {ParseStatus::ClockFailure, offset, check.cause};
  }
  return {ParseStatus::ClockFailure, offset, check.cause};
}
Uuid CanonicalUuid(std::span<const std::uint8_t, 16> native) noexcept {
  Uuid canonical{};
  constexpr std::array<std::size_t, 16> order{3, 2, 1,  0,  5,  4,  7,  6,
                                              8, 9, 10, 11, 12, 13, 14, 15};
  for (std::size_t i = 0; i < order.size(); ++i) {
    canonical[i] = native[order[i]];
  }
  return canonical;
}
bool ValidUtf8(std::span<const std::uint8_t> text) noexcept {
  std::size_t i = 0;
  while (i < text.size()) {
    const auto first = text[i++];
    if (first <= 0x7f) {
      continue;
    }
    const unsigned count = first >= 0xc2 && first <= 0xdf   ? 1
                           : first >= 0xe0 && first <= 0xef ? 2
                           : first >= 0xf0 && first <= 0xf4 ? 3
                                                            : 0;
    if (count == 0 || count > text.size() - i) {
      return false;
    }
    if ((first == 0xe0 && text[i] < 0xa0) ||
        (first == 0xed && text[i] > 0x9f) ||
        (first == 0xf0 && text[i] < 0x90) ||
        (first == 0xf4 && text[i] > 0x8f)) {
      return false;
    }
    for (unsigned continuation = 0; continuation < count; ++continuation) {
      if ((text[i++] & 0xc0) != 0x80) {
        return false;
      }
    }
  }
  return true;
}
ParseResult ParseCrashpadMetadata(std::span<const std::uint8_t> image,
                                  ParseControl& control,
                                  MetadataFacts& output) noexcept {
  output = {};
  const auto first = CheckProgress(control, 0);
  if (first.status != ParseStatus::Valid) {
    return first;
  }
  const auto result = Parse(image, control, output);
  if (result.status != ParseStatus::Valid) {
    output = {};
  }
  return result;
}
ParseResult FindMetadataUuid(const MetadataFacts& facts, const Uuid& uuid,
                             std::uint32_t& record_index) noexcept {
  record_index = 0;
  if (!Nonzero(uuid) || facts.count > kReportLimit) {
    return {ParseStatus::WrongIdentity};
  }
  for (std::uint32_t i = 0; i < facts.count; ++i) {
    if (facts.records[i].uuid == uuid) {
      record_index = i;
      return {facts.records[i].state == ReportState::Uploading
                  ? ParseStatus::Busy
                  : ParseStatus::Valid};
    }
  }
  return {ParseStatus::Missing};
}
}  // namespace sugarsubstitute::finished
