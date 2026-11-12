// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "incident_bridge.h"

#include <algorithm>
#include <string_view>

namespace sugarsubstitute::finished {
namespace {
using Bytes = std::span<const std::uint8_t>;
std::uint32_t U32(Bytes bytes, std::size_t offset) noexcept {
  std::uint32_t value = 0;
  for (std::size_t i = 0; i < 4; ++i) {
    value |= static_cast<std::uint32_t>(bytes[offset + i]) << (8 * i);
  }
  return value;
}
struct Location {
  std::uint32_t size = 0;
  std::uint32_t rva = 0;
};
Location At(Bytes bytes, std::size_t offset) noexcept {
  return {U32(bytes, offset), U32(bytes, offset + 4)};
}
bool Nonzero(Bytes bytes) noexcept {
  return std::any_of(bytes.begin(), bytes.end(),
                     [](auto byte) { return byte != 0; });
}
bool Equal(Bytes bytes, std::string_view text) noexcept {
  return bytes.size() == text.size() &&
         std::equal(bytes.begin(), bytes.end(), text.begin());
}
void Hex(Bytes source, std::span<std::uint8_t> target) noexcept {
  constexpr char digits[] = "0123456789abcdef";
  for (std::size_t i = 0; i < source.size(); ++i) {
    target[2 * i] = static_cast<std::uint8_t>(digits[source[i] >> 4]);
    target[2 * i + 1] = static_cast<std::uint8_t>(digits[source[i] & 15]);
  }
}
void HexNumber(std::uint64_t value,
               std::span<std::uint8_t, 16> output) noexcept {
  constexpr char digits[] = "0123456789abcdef";
  for (std::size_t i = 0; i < 16; ++i) {
    output[15 - i] = static_cast<std::uint8_t>(digits[value & 15]);
    value >>= 4;
  }
}
bool ValidExpected(const ExpectedIdentity& expected) noexcept {
  if (!Nonzero(expected.epoch) || !Nonzero(expected.report_uuid) ||
      expected.build_length == 0 ||
      expected.build_length > expected.build.size()) {
    return false;
  }
  if (expected.role == SubjectRole::Supervisor) {
    if (expected.instance != 0 || expected.generation != 0) return false;
  } else if (expected.role != SubjectRole::Desktop || expected.instance == 0 ||
             expected.generation == 0) {
    return false;
  }
  const auto build = Bytes(expected.build).first(expected.build_length);
  return std::all_of(build.begin(), build.end(), [](auto byte) {
    return (byte >= 'a' && byte <= 'z') || (byte >= 'A' && byte <= 'Z') ||
           (byte >= '0' && byte <= '9') || byte == '_' || byte == '-';
  });
}
struct Text {
  std::array<std::uint8_t, kValueLimit> bytes{};
  std::uint32_t length = 0;
  Bytes View() const noexcept { return Bytes(bytes).first(length); }
};
struct Dictionary {
  std::array<Text, 5> values{};
  std::uint32_t seen = 0;
};
class Reader {
 public:
  Reader(DumpSource& source, std::uint64_t size, ParseControl& control,
         const ExpectedIdentity& expected, DumpIdentityFacts& facts) noexcept
      : source_(source),
        size_(size),
        control_(control),
        expected_(expected),
        facts_(facts) {
    Hex(expected.name_space, std::span(coordinator_).first(64));
    Hex(expected.epoch, std::span(coordinator_).subspan(64));
    subject_[0] = expected.role == SubjectRole::Supervisor ? 's' : 'd';
    subject_[1] = '1';
    subject_[2] = ':';
    std::copy(coordinator_.begin(), coordinator_.end(), subject_.begin() + 3);
    subject_length_ = 99;
    if (expected.role == SubjectRole::Desktop) {
      subject_[99] = ':';
      HexNumber(expected.instance,
                std::span<std::uint8_t, 16>(subject_.data() + 100, 16));
      subject_[116] = ':';
      HexNumber(expected.generation,
                std::span<std::uint8_t, 16>(subject_.data() + 117, 16));
      subject_length_ = 133;
    }
  }
  ParseResult Parse() noexcept {
    std::array<std::uint8_t, 32> header{};
    if (!Read(0, header)) return failure_;
    if (U32(header, 0) != 0x504d444d || (U32(header, 4) & 0xffff) != 0xa793) {
      return {ParseStatus::Corrupt};
    }
    const auto count = U32(header, 8);
    const auto directory = U32(header, 12);
    if (count > kDirectoryLimit) return {ParseStatus::Oversized, 8};
    if (!Range(directory, static_cast<std::uint64_t>(count) * 12))
      return failure_;
    Location modules{}, crashpad{};
    bool has_modules = false, has_crashpad = false;
    for (std::uint32_t i = 0; i < count; ++i) {
      std::array<std::uint8_t, 12> entry{};
      const auto offset = directory + static_cast<std::uint64_t>(i) * 12;
      if (!Read(offset, entry)) return failure_;
      const auto location = At(entry, 4);
      if (!Descriptor(location)) return failure_;
      switch (U32(entry, 0)) {
        case 4:
          if (has_modules) return {ParseStatus::Corrupt, offset};
          has_modules = true;
          modules = location;
          break;
        case 0x43500001:
          if (has_crashpad) return {ParseStatus::Corrupt, offset};
          has_crashpad = true;
          crashpad = location;
          break;
      }
    }
    if (!has_modules || !has_crashpad || modules.size < 4 ||
        (crashpad.size != 56 && crashpad.size != 64))
      return {ParseStatus::Corrupt};
    std::array<std::uint8_t, 4> module_count_bytes{};
    if (!Read(modules.rva, module_count_bytes)) return failure_;
    const auto module_count = U32(module_count_bytes, 0);
    if (module_count > kModuleLimit)
      return {ParseStatus::Oversized, modules.rva};
    if (modules.size != 4 + static_cast<std::uint64_t>(module_count) * 108) {
      return {ParseStatus::Corrupt, modules.rva};
    }
    std::array<std::uint8_t, 64> info{};
    if (!Read(crashpad.rva, std::span(info).first(crashpad.size)))
      return failure_;
    if (U32(info, 0) != 1 || U32(info, 52) != 0)
      return {ParseStatus::Corrupt, crashpad.rva};
    facts_.report_uuid =
        CanonicalUuid(std::span<const std::uint8_t, 16>(info.data() + 4, 16));
    if (facts_.report_uuid != expected_.report_uuid)
      return {ParseStatus::WrongIdentity, crashpad.rva + 4};
    Dictionary process{};
    if (!ReadDictionary(At(info, 36), process)) return failure_;
    if (!Matches(process, false))
      return {ParseStatus::WrongIdentity, crashpad.rva};
    const auto links = At(info, 44);
    if (!Descriptor(links) || links.size < 4)
      return {ParseStatus::Corrupt, links.rva};
    std::array<std::uint8_t, 4> link_count_bytes{};
    if (!Read(links.rva, link_count_bytes)) return failure_;
    const auto link_count = U32(link_count_bytes, 0);
    if (link_count > kModuleLimit) return {ParseStatus::Oversized, links.rva};
    if (links.size != 4 + static_cast<std::uint64_t>(link_count) * 12) {
      return {ParseStatus::Corrupt, links.rva};
    }
    std::array<bool, kModuleLimit> indexes{};
    std::array<std::uint32_t, kModuleLimit> locations{};
    bool matched = false;
    for (std::uint32_t i = 0; i < link_count; ++i) {
      std::array<std::uint8_t, 12> link{};
      const auto offset = links.rva + 4 + static_cast<std::uint64_t>(i) * 12;
      if (!Read(offset, link)) return failure_;
      const auto index = U32(link, 0);
      const auto location = At(link, 4);
      if (index >= module_count || indexes[index])
        return {ParseStatus::Corrupt, offset};
      indexes[index] = true;
      if (std::find(locations.begin(), locations.begin() + i, location.rva) !=
          locations.begin() + i)
        return {ParseStatus::Corrupt, offset + 8};
      locations[i] = location.rva;
      if (!Descriptor(location) || location.size != 28)
        return {ParseStatus::Corrupt, offset + 4};
      std::array<std::uint8_t, 28> module_info{};
      if (!Read(location.rva, module_info)) return failure_;
      if (U32(module_info, 0) != 1 || !Descriptor(At(module_info, 4)) ||
          !Descriptor(At(module_info, 20)))
        return {ParseStatus::Corrupt, location.rva};
      Dictionary dictionary{};
      if (!ReadDictionary(At(module_info, 12), dictionary)) return failure_;
      if ((dictionary.seen & 1) != 0 &&
          Equal(dictionary.values[0].View(), "SugarSubstitute")) {
        if (matched || !Matches(dictionary, true))
          return {ParseStatus::WrongIdentity, location.rva};
        matched = true;
        facts_.module_index = index;
      } else if ((dictionary.seen & 1) == 0 && (dictionary.seen & 0x1c) != 0) {
        // A subject tuple cannot be spliced across dictionaries without its
        // product.
        return {ParseStatus::WrongIdentity, location.rva};
      }
    }
    if (!matched) return {ParseStatus::WrongIdentity};
    return CheckProgress(control_, size_);
  }

 private:
  bool Range(std::uint64_t offset, std::uint64_t bytes) noexcept {
    if (offset > size_ || bytes > size_ - offset) {
      failure_ = {ParseStatus::Corrupt, offset};
      return false;
    }
    return true;
  }
  bool Descriptor(Location location) noexcept {
    if (location.size == 0 && location.rva != 0) {
      failure_ = {ParseStatus::Corrupt, location.rva};
      return false;
    }
    return Range(location.rva, location.size);
  }
  bool Read(std::uint64_t offset, std::span<std::uint8_t> output) noexcept {
    if (!Range(offset, output.size())) return false;
    failure_ = CheckProgress(control_, offset);
    if (failure_.status != ParseStatus::Valid) return false;
    const auto result = source_.Read(offset, output);
    const auto after_read = CheckProgress(control_, offset);
    if (!result.complete) {
      failure_ = {ParseStatus::SourceFailure, offset, result.cause};
      return false;
    }
    failure_ = after_read;
    return failure_.status == ParseStatus::Valid;
  }
  bool String(std::uint32_t rva, std::uint32_t limit, Text& text) noexcept {
    std::array<std::uint8_t, 4> length{};
    if (!Read(rva, length)) return false;
    text.length = U32(length, 0);
    if (text.length > limit) {
      failure_ = {ParseStatus::Oversized, rva};
      return false;
    }
    std::array<std::uint8_t, kValueLimit + 1> buffer{};
    if (!Read(
            static_cast<std::uint64_t>(rva) + 4,
            std::span(buffer).first(static_cast<std::size_t>(text.length) + 1)))
      return false;
    const auto value = Bytes(buffer).first(text.length);
    if (buffer[text.length] != 0 || !ValidUtf8(value) ||
        std::find(value.begin(), value.end(), 0) != value.end()) {
      failure_ = {ParseStatus::Corrupt, rva};
      return false;
    }
    std::copy(value.begin(), value.end(), text.bytes.begin());
    return true;
  }
  bool ReadDictionary(Location location, Dictionary& output) noexcept {
    if (!Descriptor(location)) return false;
    if (location.size == 0) return true;
    if (location.size < 4) {
      failure_ = {ParseStatus::Corrupt, location.rva};
      return false;
    }
    ++facts_.dictionaries;
    std::array<std::uint8_t, 4> count_bytes{};
    if (!Read(location.rva, count_bytes)) return false;
    const auto count = U32(count_bytes, 0);
    if (count > kDictionaryEntryLimit) {
      failure_ = {ParseStatus::Oversized, location.rva};
      return false;
    }
    if (location.size != 4 + static_cast<std::uint64_t>(count) * 8) {
      failure_ = {ParseStatus::Corrupt, location.rva};
      return false;
    }
    constexpr std::array<std::string_view, 5> keys{"product", "build", "role",
                                                   "coordinator", "subject"};
    for (std::uint32_t i = 0; i < count; ++i) {
      std::array<std::uint8_t, 8> entry{};
      const auto offset = location.rva + 4 + static_cast<std::uint64_t>(i) * 8;
      if (!Read(offset, entry)) return false;
      Text key{}, value{};
      if (!String(U32(entry, 0), kKeyLimit, key) ||
          !String(U32(entry, 4), kValueLimit, value))
        return false;
      ++facts_.entries;
      for (std::size_t k = 0; k < keys.size(); ++k) {
        if (!Equal(key.View(), keys[k])) continue;
        const auto bit = 1u << k;
        if ((output.seen & bit) != 0) {
          failure_ = {ParseStatus::WrongIdentity, offset};
          return false;
        }
        if (++owned_annotations_ > kOwnedAnnotationLimit) {
          failure_ = {ParseStatus::Oversized, offset};
          return false;
        }
        output.seen |= bit;
        output.values[k] = value;
      }
    }
    return true;
  }
  bool Matches(const Dictionary& dictionary, bool complete) const noexcept {
    if (complete && dictionary.seen != 31) return false;
    for (std::size_t i = 0; i < dictionary.values.size(); ++i) {
      if ((dictionary.seen & (1u << i)) == 0) continue;
      const auto value = dictionary.values[i].View();
      switch (i) {
        case 0:
          if (!Equal(value, "SugarSubstitute")) return false;
          break;
        case 1:
          if (!std::ranges::equal(
                  value, Bytes(expected_.build).first(expected_.build_length)))
            return false;
          break;
        case 2:
          if (!Equal(value, expected_.role == SubjectRole::Supervisor
                                ? "supervisor"
                                : "desktop"))
            return false;
          break;
        case 3:
          if (!std::ranges::equal(value, coordinator_)) return false;
          break;
        case 4:
          if (!std::ranges::equal(value,
                                  Bytes(subject_).first(subject_length_)))
            return false;
          break;
      }
    }
    return true;
  }
  DumpSource& source_;
  const std::uint64_t size_;
  ParseControl& control_;
  const ExpectedIdentity& expected_;
  DumpIdentityFacts& facts_;
  ParseResult failure_{};
  std::uint32_t owned_annotations_ = 0;
  std::array<std::uint8_t, 96> coordinator_{};
  std::array<std::uint8_t, 133> subject_{};
  std::uint32_t subject_length_ = 0;
};
}  // namespace

ParseResult ParseCrashpadIdentity(DumpSource& source, std::uint64_t file_size,
                                  const ExpectedIdentity& expected,
                                  ParseControl& control,
                                  DumpIdentityFacts& output) noexcept {
  output = {};
  const auto first = CheckProgress(control, 0);
  if (first.status != ParseStatus::Valid) return first;
  if (file_size > kDumpLimit) return {ParseStatus::Oversized};
  if (!ValidExpected(expected)) return {ParseStatus::WrongIdentity};
  Reader reader(source, file_size, control, expected, output);
  const auto result = reader.Parse();
  if (result.status != ParseStatus::Valid) output = {};
  return result;
}
}  // namespace sugarsubstitute::finished
