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
  std::vector<std::uint8_t> image(16 + static_cast<std::size_t>(count) * 64);
  Put(image, 0, 0x43504144);
  Put(image, 4, 1);
  Put(image, 8, count);
  std::vector<std::uint8_t> table;
  for (std::uint32_t i = 0; i < count; ++i) {
    const auto offset = 16 + static_cast<std::size_t>(i) * 64;
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
    Put(image, offset + 52, i % 2 == 0 ? 0 : 2);
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
bool GoldenUuidAndSelection() {
  const auto image = Image();
  Control control;
  sf::MetadataFacts facts;
  if (sf::ParseCrashpadMetadata(image, control, facts).status !=
          sf::ParseStatus::Valid ||
      facts.count != 1 || facts.records[0].uuid != kUuid ||
      facts.string_table_offset != 80 || facts.records[0].offset != 16) {
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
  for (std::size_t bytes : {0u, 15u, 16u, 79u, 80u}) {
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
  Put(image, 68, 3);
  if (!Status(image, sf::ParseStatus::Corrupt)) {
    return false;
  }
  image = Image();
  image[72] = 4;
  if (!Status(image, sf::ParseStatus::Corrupt)) {
    return false;
  }
  image = Image();
  image[79] = 1;
  return Status(image, sf::ParseStatus::Corrupt);
}
bool UploadingIsBusy() {
  auto image = Image();
  Put(image, 68, 1);
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
  std::copy_n(image.begin() + 16, 16, image.begin() + 80);
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
    image[80 + character] = 'X';
    if (!Status(image, sf::ParseStatus::Corrupt)) {
      return false;
    }
  }
  auto image = Image();
  image[120] = 'x';
  return Status(image, sf::ParseStatus::Corrupt);
}
bool MissingNulAndMalformedUtf8() {
  auto image = Image();
  image.back() = 'x';
  if (!Status(image, sf::ParseStatus::Corrupt)) {
    return false;
  }
  image = Image();
  image[123] = 0xc0;
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
        result.offset != 80 || facts.count != 0 || control.calls != 3) {
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
