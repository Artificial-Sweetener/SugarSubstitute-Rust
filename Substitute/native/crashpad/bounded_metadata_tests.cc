// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include <algorithm>
#include <cstdio>
#include <string>
#include <vector>

#include "bounded_metadata.h"

namespace sf = sugarsubstitute::finished;
namespace {
class Control final : public sf::ParseControl {
 public:
  sf::Checkpoint Check() noexcept override {
    ++calls;
    return calls == interrupt_at ? interruption : sf::Checkpoint{};
  }
  std::uint32_t calls = 0;
  std::uint32_t interrupt_at = 0;
  sf::Checkpoint interruption{};
};
void Put(std::vector<std::uint8_t>& image, std::size_t offset,
         std::uint32_t value) {
  for (unsigned i = 0; i < 4; ++i) {
    image[offset + i] = static_cast<std::uint8_t>(value >> (8 * i));
  }
}
const sf::Uuid kUuid{0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef,
                     0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88};
std::vector<std::uint8_t> Image(std::uint32_t count = 1) {
  std::vector<std::uint8_t> image(16 + static_cast<std::size_t>(count) * 56);
  Put(image, 0, 0x43504144);
  Put(image, 4, 1);
  Put(image, 8, count);
  std::vector<std::uint8_t> table;
  for (std::uint32_t i = 0; i < count; ++i) {
    const auto offset = 16 + static_cast<std::size_t>(i) * 56;
    std::array<std::uint8_t, 16> raw{0x78, 0x56, 0x34, 0x12, 0xab, 0x90,
                                     0xef, 0xcd, 0x11, 0x22, 0x33, 0x44,
                                     0x55, 0x66, 0x77, 0x88};
    std::string name = "12345678-90ab-cdef-1122-334455667788.dmp";
    if (count != 1) {
      raw[15] = static_cast<std::uint8_t>(i);
      constexpr char hex[] = "0123456789abcdef";
      name[34] = hex[i >> 4];
      name[35] = hex[i & 15];
    }
    std::copy(raw.begin(), raw.end(), image.begin() + offset);
    Put(image, offset + 16, static_cast<std::uint32_t>(table.size()));
    table.insert(table.end(), name.begin(), name.end());
    table.push_back(0);
    Put(image, offset + 20, static_cast<std::uint32_t>(table.size()));
    const std::array<std::uint8_t, 5> upload_id{'i', 'd', 0xe2, 0x82, 0xac};
    table.insert(table.end(), upload_id.begin(), upload_id.end());
    table.push_back(0);
    Put(image, offset + 44, i % 2 == 0 ? 0 : 2);
  }
  image.insert(image.end(), table.begin(), table.end());
  return image;
}
bool Status(std::span<const std::uint8_t> image, sf::ParseStatus expected) {
  Control control;
  sf::MetadataFacts facts;
  facts.count = 99;
  const auto parsed = sf::ParseCrashpadMetadata(image, control, facts);
  return parsed.status == expected &&
         (expected == sf::ParseStatus::Valid || facts.count == 0);
}
bool ActualPinnedProducerMetadata() {
  // Exact metadata emitted by the pinned Windows producer during Source82.
  // UUID/time/string bytes are observations and grant no file or native authority.
  constexpr std::array<std::uint8_t, 114> image{
      0x44, 0x41, 0x50, 0x43, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
      0x00, 0x00, 0x00, 0x00, 0x37, 0xd6, 0x82, 0x16, 0x06, 0x3d, 0xa2, 0x4f,
      0xad, 0xb9, 0x2b, 0x8d, 0xf3, 0x8a, 0x78, 0xc3, 0x00, 0x00, 0x00, 0x00,
      0x29, 0x00, 0x00, 0x00, 0x8d, 0x9d, 0xd0, 0x6a, 0x00, 0x00, 0x00, 0x00,
      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
      0x31, 0x36, 0x38, 0x32, 0x64, 0x36, 0x33, 0x37, 0x2d, 0x33, 0x64, 0x30,
      0x36, 0x2d, 0x34, 0x66, 0x61, 0x32, 0x2d, 0x61, 0x64, 0x62, 0x39, 0x2d,
      0x32, 0x62, 0x38, 0x64, 0x66, 0x33, 0x38, 0x61, 0x37, 0x38, 0x63, 0x33,
      0x2e, 0x64, 0x6d, 0x70, 0x00, 0x00,
  };
  constexpr sf::Uuid expected{0x16, 0x82, 0xd6, 0x37, 0x3d, 0x06, 0x4f, 0xa2, 0xad, 0xb9, 0x2b, 0x8d, 0xf3, 0x8a, 0x78, 0xc3};
  Control control; sf::MetadataFacts facts;
  if (sf::ParseCrashpadMetadata(image, control, facts).status != sf::ParseStatus::Valid ||
      facts.count != 1 || facts.string_table_offset != 72 || facts.records[0].offset != 16 ||
      facts.records[0].uuid != expected || facts.records[0].state != sf::ReportState::Pending) return false;
  std::vector<std::uint8_t> padded(image.begin(), image.end());
  padded.insert(padded.begin() + 72, 8, 0);
  return Status(padded, sf::ParseStatus::Corrupt);
}
bool GoldenUuidAndSelection() {
  const auto image = Image();
  Control control;
  sf::MetadataFacts facts;
  if (sf::ParseCrashpadMetadata(image, control, facts).status !=
          sf::ParseStatus::Valid ||
      facts.count != 1 || facts.records[0].uuid != kUuid ||
      facts.string_table_offset != 72 || facts.records[0].offset != 16) {
    return false;
  }
  std::uint32_t index = 55;
  return sf::FindMetadataUuid(facts, kUuid, index).status ==
             sf::ParseStatus::Valid &&
         index == 0;
}
bool MaximumRecords() {
  const auto image = Image(sf::kReportLimit);
  Control control;
  sf::MetadataFacts facts;
  return sf::ParseCrashpadMetadata(image, control, facts).status ==
             sf::ParseStatus::Valid &&
         facts.count == sf::kReportLimit &&
         control.calls == sf::kReportLimit + 2;
}
bool OversizedRecords() {
  auto image = Image(0);
  Put(image, 8, sf::kReportLimit + 1);
  return Status(image, sf::ParseStatus::Oversized);
}
bool MaximumImageAndPlusOne() {
  auto image = Image();
  image.resize(static_cast<std::size_t>(sf::kMetadataLimit), 0);
  if (!Status(image, sf::ParseStatus::Valid)) {
    return false;
  }
  image.push_back(0);
  return Status(image, sf::ParseStatus::Oversized);
}
bool Truncation() {
  const auto image = Image();
  for (std::size_t bytes : {0u, 15u, 16u, 71u, 72u}) {
    if (!Status(std::span(image).first(bytes), sf::ParseStatus::Corrupt)) {
      return false;
    }
  }
  return true;
}
bool BadHeader() {
  for (std::size_t offset : {0u, 4u, 12u}) {
    auto image = Image();
    Put(image, offset, 2);
    if (!Status(image, sf::ParseStatus::Corrupt)) {
      return false;
    }
  }
  return true;
}
bool BadStateAndReservedAttributes() {
  auto image = Image();
  Put(image, 60, 3);
  if (!Status(image, sf::ParseStatus::Corrupt)) {
    return false;
  }
  image = Image();
  image[64] = 4;
  if (!Status(image, sf::ParseStatus::Corrupt)) {
    return false;
  }
  image = Image();
  image[71] = 1;
  return Status(image, sf::ParseStatus::Corrupt);
}
bool UploadingIsBusy() {
  auto image = Image();
  Put(image, 60, 1);
  Control control;
  sf::MetadataFacts facts;
  std::uint32_t index = 0;
  return sf::ParseCrashpadMetadata(image, control, facts).status ==
             sf::ParseStatus::Valid &&
         sf::FindMetadataUuid(facts, kUuid, index).status ==
             sf::ParseStatus::Busy;
}
bool ZeroAndDuplicateUuid() {
  auto image = Image();
  std::fill(image.begin() + 16, image.begin() + 32, std::uint8_t{0});
  if (!Status(image, sf::ParseStatus::WrongIdentity)) {
    return false;
  }
  image = Image(2);
  std::copy_n(image.begin() + 16, 16, image.begin() + 72);
  return Status(image, sf::ParseStatus::WrongIdentity);
}
bool BadStringIndexes() {
  for (auto index : {1u, 47u, 0xffffffffu}) {
    auto image = Image();
    Put(image, 32, index);
    if (!Status(image, sf::ParseStatus::Corrupt)) {
      return false;
    }
    image = Image();
    Put(image, 36, index);
    if (!Status(image, sf::ParseStatus::Corrupt)) {
      return false;
    }
  }
  return true;
}
bool PathAndUuidMismatch() {
  for (std::size_t character : {0u, 8u, 36u, 39u}) {
    auto image = Image();
    image[72 + character] = 'X';
    if (!Status(image, sf::ParseStatus::Corrupt)) {
      return false;
    }
  }
  auto image = Image();
  image[112] = 'x';
  return Status(image, sf::ParseStatus::Corrupt);
}
bool MissingNulAndMalformedUtf8() {
  auto image = Image();
  image.back() = 'x';
  if (!Status(image, sf::ParseStatus::Corrupt)) {
    return false;
  }
  image = Image();
  image[115] = 0xc0;
  if (!Status(image, sf::ParseStatus::Corrupt)) {
    return false;
  }
  image = Image(0);
  image.push_back('x');
  return Status(image, sf::ParseStatus::Corrupt);
}
bool ExactMissingIsNotCorruption() {
  const auto image = Image();
  Control control;
  sf::MetadataFacts facts;
  if (sf::ParseCrashpadMetadata(image, control, facts).status !=
      sf::ParseStatus::Valid) {
    return false;
  }
  auto different = kUuid;
  different[15] = 1;
  std::uint32_t index = 5;
  return sf::FindMetadataUuid(facts, different, index).status ==
             sf::ParseStatus::Missing &&
         index == 0 &&
         sf::FindMetadataUuid(facts, {}, index).status ==
             sf::ParseStatus::WrongIdentity;
}
bool InterruptionPreservesCauseAndClearsPartialFacts() {
  const auto image = Image(2);
  for (auto interruption :
       {sf::Interruption::DeadlineElapsed, sf::Interruption::Cancelled,
        sf::Interruption::ClockFailure}) {
    Control control;
    control.interrupt_at = 3;
    control.interruption = {interruption, 12345};
    sf::MetadataFacts facts;
    const auto result = sf::ParseCrashpadMetadata(image, control, facts);
    const auto expected = interruption == sf::Interruption::DeadlineElapsed
                              ? sf::ParseStatus::DeadlineElapsed
                          : interruption == sf::Interruption::Cancelled
                              ? sf::ParseStatus::Cancelled
                              : sf::ParseStatus::ClockFailure;
    if (result.status != expected || result.cause != 12345 ||
        result.offset != 72 || facts.count != 0 || control.calls != 3) {
      return false;
    }
  }
  return true;
}
bool EmptyMetadataIsValid() { return Status(Image(0), sf::ParseStatus::Valid); }
bool Utf8ScalarBoundaries() {
  for (const std::vector<std::uint8_t>& text :
       {std::vector<std::uint8_t>{0x7f, 0xc2, 0x80, 0xdf, 0xbf},
        {0xe0, 0xa0, 0x80, 0xed, 0x9f, 0xbf},
        {0xf0, 0x90, 0x80, 0x80, 0xf4, 0x8f, 0xbf, 0xbf}}) {
    auto image = Image(0);
    image.insert(image.end(), text.begin(), text.end());
    image.push_back(0);
    if (!Status(image, sf::ParseStatus::Valid)) return false;
  }
  for (const std::vector<std::uint8_t>& text : {std::vector<std::uint8_t>{0x80},
                                                {0xc0, 0x80},
                                                {0xc1, 0xbf},
                                                {0xe0, 0x9f, 0xbf},
                                                {0xed, 0xa0, 0x80},
                                                {0xf0, 0x8f, 0xbf, 0xbf},
                                                {0xf4, 0x90, 0x80, 0x80},
                                                {0xf5, 0x80, 0x80, 0x80},
                                                {0xe2, 0x82}}) {
    auto image = Image(0);
    image.insert(image.end(), text.begin(), text.end());
    image.push_back(0);
    if (!Status(image, sf::ParseStatus::Corrupt)) return false;
  }
  return true;
}
struct Test {
  const char* name;
  bool (*run)();
};
}  // namespace
int main() {
  const Test tests[]{
      {"actual pinned producer56-byte record and unsupported padding", ActualPinnedProducerMetadata},
      {"golden asymmetric UUID and exact selection", GoldenUuidAndSelection},
      {"256 actual records", MaximumRecords},
      {"257 claimed records without allocation", OversizedRecords},
      {"1MiB image and plus one", MaximumImageAndPlusOne},
      {"header and record truncation", Truncation},
      {"unknown magic version reserved header", BadHeader},
      {"unknown state attributes padding", BadStateAndReservedAttributes},
      {"actual uploading record is Busy", UploadingIsBusy},
      {"zero and duplicate UUID", ZeroAndDuplicateUuid},
      {"out of range and interior string references", BadStringIndexes},
      {"exact canonical basename UUID equality", PathAndUuidMismatch},
      {"unterminated and malformed UTF8 table", MissingNulAndMalformedUtf8},
      {"complete scan exact Missing", ExactMissingIsNotCorruption},
      {"deadline cancellation clock causes stop partial publication",
       InterruptionPreservesCauseAndClearsPartialFacts},
      {"actual empty metadata", EmptyMetadataIsValid},
      {"UTF8 scalar limits overlong surrogates truncation",
       Utf8ScalarBoundaries},
  };
  std::size_t failed = 0;
  for (const auto& test : tests) {
    const bool passed = test.run();
    std::printf("%s: %s\n", passed ? "PASS" : "FAIL", test.name);
    failed += passed ? 0u : 1u;
  }
  std::printf("selected=%zu passed=%zu failed=%zu ignored=0\n",
              std::size(tests), std::size(tests) - failed, failed);
  return failed == 0 ? 0 : 1;
}
