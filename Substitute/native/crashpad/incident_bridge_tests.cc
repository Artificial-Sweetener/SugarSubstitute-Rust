// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include <algorithm>
#include <cstdio>
#include <string>
#include <utility>
#include <vector>

#include "incident_bridge.h"

namespace {
namespace sf = sugarsubstitute::finished;
using Pair = std::pair<std::string, std::string>;
using Pairs = std::vector<Pair>;
constexpr std::array<std::uint8_t, 16> kNativeUuid{
    0x78, 0x56, 0x34, 0x12, 0xab, 0x90, 0xef, 0xcd,
    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88};
constexpr sf::Uuid kCanonicalUuid{0x12, 0x34, 0x56, 0x78, 0x90, 0xab,
                                  0xcd, 0xef, 0x11, 0x22, 0x33, 0x44,
                                  0x55, 0x66, 0x77, 0x88};
class Control final : public sf::ParseControl {
 public:
  sf::Checkpoint Check() noexcept override {
    return ++calls == interrupt_at ? interruption : sf::Checkpoint{};
  }
  std::uint32_t calls = 0;
  std::uint32_t interrupt_at = 0;
  sf::Checkpoint interruption{};
};
class Source final : public sf::DumpSource {
 public:
  sf::ReadResult Read(std::uint64_t offset,
                      std::span<std::uint8_t> out) noexcept override {
    ++calls;
    largest_read = std::max(largest_read, out.size());
    if (calls == fail_at) return {false, 9981};
    if (offset > image.size() || out.size() > image.size() - offset)
      return {false, 9982};
    std::copy_n(image.begin() + static_cast<std::size_t>(offset), out.size(),
                out.begin());
    return {true};
  }
  std::uint32_t Allocate(std::size_t bytes) {
    const auto offset = static_cast<std::uint32_t>(image.size());
    image.resize(image.size() + bytes);
    return offset;
  }
  void Put(std::uint32_t offset, std::uint32_t value) {
    for (std::size_t i = 0; i < 4; ++i)
      image[offset + i] = static_cast<std::uint8_t>(value >> (8 * i));
  }
  std::uint32_t String(const std::string& value) {
    const auto offset = Allocate(value.size() + 5);
    Put(offset, static_cast<std::uint32_t>(value.size()));
    std::copy(value.begin(), value.end(), image.begin() + offset + 4);
    return offset;
  }
  std::uint32_t Dictionary(const Pairs& pairs) {
    const auto offset = Allocate(4 + pairs.size() * 8);
    Put(offset, static_cast<std::uint32_t>(pairs.size()));
    for (std::size_t i = 0; i < pairs.size(); ++i) {
      const auto entry = offset + 4 + static_cast<std::uint32_t>(i) * 8;
      Put(entry, String(pairs[i].first));
      Put(entry + 4, String(pairs[i].second));
    }
    return offset;
  }
  std::vector<std::uint8_t> image;
  std::uint32_t calls = 0;
  std::uint32_t fail_at = 0;
  std::size_t largest_read = 0;
};
sf::ExpectedIdentity Expected(bool desktop = false) {
  sf::ExpectedIdentity expected;
  expected.name_space.fill(0x12);
  expected.epoch.fill(0xab);
  expected.build[0] = 'b';
  expected.build[1] = '7';
  expected.build_length = 2;
  expected.report_uuid = kCanonicalUuid;
  if (desktop) {
    expected.role = sf::SubjectRole::Desktop;
    expected.instance = 3;
    expected.generation = 15;
  }
  return expected;
}
std::string Coordinator() {
  std::string text;
  for (int i = 0; i < 32; ++i) text += "12";
  for (int i = 0; i < 16; ++i) text += "ab";
  return text;
}
Pairs Host(bool desktop = false) {
  const auto coordinator = Coordinator();
  const auto subject =
      desktop ? "d1:" + coordinator + ":0000000000000003:000000000000000f"
              : "s1:" + coordinator;
  return {{"product", "SugarSubstitute"},
          {"build", "b7"},
          {"role", desktop ? "desktop" : "supervisor"},
          {"coordinator", coordinator},
          {"subject", subject}};
}
struct Fixture {
  explicit Fixture(std::vector<Pairs> dictionaries = {Host()},
                   Pairs process = {{"product", "SugarSubstitute"},
                                    {"build", "b7"}},
                   std::uint32_t directories = 2) {
    source.Allocate(32);
    directory = source.Allocate(static_cast<std::size_t>(directories) * 12);
    const auto count = static_cast<std::uint32_t>(dictionaries.size());
    modules = source.Allocate(4 + static_cast<std::size_t>(count) * 108);
    crashpad = source.Allocate(64);
    links = source.Allocate(4 + static_cast<std::size_t>(count) * 12);
    source.Put(0, 0x504d444d);
    source.Put(4, 0xa793);
    source.Put(8, directories);
    source.Put(12, directory);
    source.Put(directory, 4);
    source.Put(directory + 4, 4 + count * 108);
    source.Put(directory + 8, modules);
    source.Put(directory + 12, 0x43500001);
    source.Put(directory + 16, 64);
    source.Put(directory + 20, crashpad);
    source.Put(modules, count);
    source.Put(crashpad, 1);
    std::copy(kNativeUuid.begin(), kNativeUuid.end(),
              source.image.begin() + crashpad + 4);
    const auto process_dictionary = source.Dictionary(process);
    source.Put(crashpad + 36,
               4 + static_cast<std::uint32_t>(process.size()) * 8);
    source.Put(crashpad + 40, process_dictionary);
    source.Put(crashpad + 44, 4 + count * 12);
    source.Put(crashpad + 48, links);
    source.Put(links, count);
    for (std::uint32_t i = 0; i < count; ++i) {
      const auto info = source.Allocate(28);
      infos.push_back(info);
      source.Put(info, 1);
      const auto dictionary = source.Dictionary(dictionaries[i]);
      dicts.push_back(dictionary);
      source.Put(info + 12,
                 4 + static_cast<std::uint32_t>(dictionaries[i].size()) * 8);
      source.Put(info + 16, dictionary);
      source.Put(links + 4 + i * 12, i);
      source.Put(links + 8 + i * 12, 28);
      source.Put(links + 12 + i * 12, info);
    }
  }
  sf::ParseResult Parse(sf::ExpectedIdentity expected = Expected(),
                        std::uint64_t size = 0) {
    return sf::ParseCrashpadIdentity(source,
                                     size == 0 ? source.image.size() : size,
                                     expected, control, facts);
  }
  bool Status(sf::ParseStatus wanted,
              sf::ExpectedIdentity expected = Expected()) {
    const auto result = Parse(expected);
    return result.status == wanted &&
           (wanted == sf::ParseStatus::Valid ||
            (facts.report_uuid == sf::Uuid{} && facts.dictionaries == 0 &&
             facts.entries == 0));
  }
  Source source;
  Control control;
  sf::DumpIdentityFacts facts;
  std::uint32_t directory = 0, modules = 0, crashpad = 0, links = 0;
  std::vector<std::uint32_t> infos, dicts;
};
std::uint32_t Get(const Source& source, std::uint32_t offset) {
  std::uint32_t value = 0;
  for (std::size_t i = 0; i < 4; ++i)
    value |= static_cast<std::uint32_t>(source.image[offset + i]) << (8 * i);
  return value;
}
bool GoldenSupervisor() {
  Fixture fixture;
  return fixture.Status(sf::ParseStatus::Valid) &&
         fixture.facts.report_uuid == kCanonicalUuid &&
         fixture.facts.module_index == 0 && fixture.facts.dictionaries == 2 &&
         fixture.facts.entries == 7 && fixture.source.largest_read <= 257;
}
bool GoldenDesktop() {
  Fixture fixture({Host(true)});
  if (!fixture.Status(sf::ParseStatus::Valid, Expected(true))) return false;
  auto expected = Expected(true);
  ++expected.generation;
  return fixture.Status(sf::ParseStatus::WrongIdentity, expected);
}
bool GlobalDoesNotCompleteModule() {
  auto partial = Host();
  partial.pop_back();
  Fixture fixture({partial}, Host());
  return fixture.Status(sf::ParseStatus::WrongIdentity);
}
bool GlobalContradiction() {
  Fixture fixture({Host()},
                  {{"product", "SugarSubstitute"}, {"build", "other"}});
  return fixture.Status(sf::ParseStatus::WrongIdentity);
}
bool DuplicateHostModules() {
  Fixture fixture({Host(), Host()});
  return fixture.Status(sf::ParseStatus::WrongIdentity);
}
bool OwnedDuplicateKey() {
  auto pairs = Host();
  pairs.emplace_back("subject", pairs.back().second);
  Fixture fixture({pairs});
  return fixture.Status(sf::ParseStatus::WrongIdentity);
}
bool ForeignModuleAndUnicode() {
  Fixture fixture(std::vector<Pairs>{Pairs{{"product", "Other"},
                                           {"role", "other"},
                                           {"note", "\xf0\x9f\x8c\x8d"}},
                                     Host()});
  return fixture.Status(sf::ParseStatus::Valid) &&
         fixture.facts.module_index == 1;
}
bool MissingTupleAndSplice() {
  Fixture missing({{}});
  auto first = Host();
  first.resize(3);
  Fixture splice(
      {first,
       {{"coordinator", Coordinator()}, {"subject", "s1:" + Coordinator()}}});
  return missing.Status(sf::ParseStatus::WrongIdentity) &&
         splice.Status(sf::ParseStatus::WrongIdentity);
}
bool ExpectedValidation() {
  Fixture fixture;
  for (int kind = 0; kind < 7; ++kind) {
    auto expected = Expected();
    switch (kind) {
      case 0:
        expected.epoch.fill(0);
        break;
      case 1:
        expected.report_uuid.fill(0);
        break;
      case 2:
        expected.build_length = 0;
        break;
      case 3:
        expected.build_length = 65;
        break;
      case 4:
        expected.build[0] = '/';
        break;
      case 5:
        expected.instance = 1;
        break;
      case 6:
        expected.role = sf::SubjectRole::Desktop;
        break;
    }
    if (!fixture.Status(sf::ParseStatus::WrongIdentity, expected)) return false;
  }
  return fixture.source.calls == 0;
}
bool ExactFullIdentity() {
  for (int kind = 0; kind < 4; ++kind) {
    Fixture fixture;
    auto expected = Expected();
    switch (kind) {
      case 0:
        expected.name_space[31] ^= 1;
        break;
      case 1:
        expected.epoch[15] ^= 1;
        break;
      case 2:
        expected.build[1] = '8';
        break;
      case 3:
        expected.report_uuid[15] ^= 1;
        break;
    }
    if (!fixture.Status(sf::ParseStatus::WrongIdentity, expected)) return false;
  }
  return true;
}
bool HeaderAndDirectoryBounds() {
  Fixture maximum({Host()}, {{"product", "SugarSubstitute"}}, 128);
  if (!maximum.Status(sf::ParseStatus::Valid)) return false;
  for (const auto value : {129u, 0xffffffffu}) {
    Fixture fixture;
    fixture.source.Put(8, value);
    if (!fixture.Status(sf::ParseStatus::Oversized)) return false;
  }
  Fixture overflow;
  overflow.source.Put(12, 0xfffffff0);
  return overflow.Status(sf::ParseStatus::Corrupt);
}
bool RequiredAndDuplicateStreams() {
  for (const auto type : {0u, 4u}) {
    Fixture fixture;
    fixture.source.Put(fixture.directory + 12, type);
    if (!fixture.Status(sf::ParseStatus::Corrupt)) return false;
  }
  Fixture unknown;
  unknown.source.Put(0, 0);
  return unknown.Status(sf::ParseStatus::Corrupt);
}
bool StreamRangesVersionsAndReserved() {
  for (int kind = 0; kind < 6; ++kind) {
    Fixture fixture;
    switch (kind) {
      case 0:
        fixture.source.Put(fixture.directory + 16, 65);
        break;
      case 1:
        fixture.source.Put(fixture.crashpad, 2);
        break;
      case 2:
        fixture.source.Put(fixture.crashpad + 52, 1);
        break;
      case 3:
        fixture.source.Put(fixture.infos[0], 2);
        break;
      case 4:
        fixture.source.Put(fixture.infos[0] + 8, 1);
        break;
      case 5:
        fixture.source.Put(fixture.infos[0] + 20, 0xffffffff);
        break;
    }
    if (!fixture.Status(sf::ParseStatus::Corrupt)) return false;
  }
  Fixture legacy;
  legacy.source.Put(legacy.directory + 16, 56);
  return legacy.Status(sf::ParseStatus::Valid);
}
bool ModuleAndLinkLimits() {
  std::vector<Pairs> dictionaries(1024);
  dictionaries.back() = Host();
  Fixture maximum(dictionaries);
  if (!maximum.Status(sf::ParseStatus::Valid) ||
      maximum.facts.module_index != 1023 ||
      maximum.facts.dictionaries != 1025 || maximum.source.largest_read > 257)
    return false;
  for (const bool links : {false, true}) {
    Fixture fixture;
    fixture.source.Put(links ? fixture.links : fixture.modules, 1025);
    if (!fixture.Status(sf::ParseStatus::Oversized)) return false;
  }
  return true;
}
bool LinkIdentityAndExactSizes() {
  for (int kind = 0; kind < 5; ++kind) {
    Fixture fixture({Host(), {}});
    switch (kind) {
      case 0:
        fixture.source.Put(fixture.links + 16, 0);
        break;
      case 1:
        fixture.source.Put(fixture.links + 16, 2);
        break;
      case 2:
        fixture.source.Put(fixture.links + 24, fixture.infos[0]);
        break;
      case 3:
        fixture.source.Put(fixture.links + 8, 27);
        break;
      case 4:
        fixture.source.Put(fixture.directory + 4, 4);
        break;
    }
    if (!fixture.Status(sf::ParseStatus::Corrupt)) return false;
  }
  return true;
}
bool DictionaryEntryLimits() {
  auto pairs = Host();
  while (pairs.size() < 32) pairs.emplace_back("note", "value");
  Fixture maximum({pairs});
  if (!maximum.Status(sf::ParseStatus::Valid)) return false;
  Fixture oversized;
  oversized.source.Put(oversized.dicts[0], 33);
  return oversized.Status(sf::ParseStatus::Oversized);
}
bool OwnedAnnotationBudget() {
  std::vector<Pairs> dictionaries(13,
                                  {{"product", "Other"}, {"build", "other"}});
  dictionaries.push_back(Host());
  Fixture maximum(dictionaries, {{"product", "SugarSubstitute"}});
  if (!maximum.Status(sf::ParseStatus::Valid)) return false;
  dictionaries.insert(dictionaries.begin(),
                      Pairs{{"product", "Other"}, {"build", "other"}});
  Fixture oversized(dictionaries, {{"product", "SugarSubstitute"}});
  return oversized.Status(sf::ParseStatus::Oversized);
}
bool StringBoundaries() {
  auto pairs = Host();
  pairs.emplace_back(std::string(64, 'k'), std::string(256, 'v'));
  Fixture maximum({pairs});
  if (!maximum.Status(sf::ParseStatus::Valid) ||
      maximum.source.largest_read != 257)
    return false;
  for (const bool key : {false, true}) {
    Fixture fixture;
    const auto rva = Get(fixture.source, fixture.dicts[0] + (key ? 4 : 8));
    fixture.source.Put(rva, key ? 65 : 257);
    if (!fixture.Status(sf::ParseStatus::Oversized)) return false;
  }
  return true;
}
bool Utf8NulAndRvaBounds() {
  for (int kind = 0; kind < 4; ++kind) {
    Fixture fixture;
    const auto entry = fixture.dicts[0] + 8;
    const auto rva = Get(fixture.source, entry);
    switch (kind) {
      case 0:
        fixture.source.image[rva + 4] = 0xc0;
        break;
      case 1:
        fixture.source.image[rva + 4] = 0;
        break;
      case 2:
        fixture.source.image[rva + 4 + Get(fixture.source, rva)] = 1;
        break;
      case 3:
        fixture.source.Put(entry, 0xffffffff);
        break;
    }
    if (!fixture.Status(sf::ParseStatus::Corrupt)) return false;
  }
  return true;
}
bool DumpSizeBound() {
  Fixture maximum;
  if (maximum.Parse(Expected(), sf::kDumpLimit).status !=
      sf::ParseStatus::Valid)
    return false;
  Fixture oversized;
  return oversized.Parse(Expected(), sf::kDumpLimit + 1).status ==
             sf::ParseStatus::Oversized &&
         oversized.source.calls == 0;
}
bool TruncatedRanges() {
  for (const auto size : {1u, 31u, 32u, 55u, 100u, 250u}) {
    Fixture fixture;
    if (fixture.Parse(Expected(), size).status != sf::ParseStatus::Corrupt ||
        fixture.facts.entries != 0)
      return false;
  }
  return true;
}
bool SourceFailurePreservesCause() {
  Fixture golden;
  if (!golden.Status(sf::ParseStatus::Valid)) return false;
  for (const auto ordinal :
       {1u, golden.source.calls / 2, golden.source.calls}) {
    Fixture fixture;
    fixture.source.fail_at = ordinal;
    const auto result = fixture.Parse();
    if (result.status != sf::ParseStatus::SourceFailure ||
        result.cause != 9981 || fixture.source.calls != ordinal ||
        fixture.facts.entries != 0)
      return false;
  }
  return true;
}
bool FailedReadStillObservesPostControl() {
  Fixture golden;
  if (!golden.Status(sf::ParseStatus::Valid)) return false;
  for (const auto ordinal :
       {1u, golden.source.calls / 2, golden.source.calls}) {
    for (const auto interruption :
         {sf::Interruption::None, sf::Interruption::DeadlineElapsed,
          sf::Interruption::Cancelled, sf::Interruption::ClockFailure}) {
      Fixture fixture;
      fixture.source.fail_at = ordinal;
      fixture.control.interrupt_at = 2 * ordinal + 1;
      fixture.control.interruption = {interruption, 6671};
      const auto result = fixture.Parse();
      if (result.status != sf::ParseStatus::SourceFailure ||
          result.cause != 9981 || fixture.source.calls != ordinal ||
          fixture.control.calls != fixture.control.interrupt_at ||
          fixture.facts.entries != 0 || fixture.facts.report_uuid != sf::Uuid{})
        return false;
    }
  }
  return true;
}
bool OriginalInterruptionPreservesCause() {
  Fixture golden;
  if (!golden.Status(sf::ParseStatus::Valid)) return false;
  for (const auto interruption :
       {sf::Interruption::DeadlineElapsed, sf::Interruption::Cancelled,
        sf::Interruption::ClockFailure}) {
    for (const auto ordinal : {1u, 3u, golden.control.calls}) {
      Fixture fixture;
      fixture.control.interrupt_at = ordinal;
      fixture.control.interruption = {interruption, 6671};
      const auto result = fixture.Parse();
      const auto status = interruption == sf::Interruption::DeadlineElapsed
                              ? sf::ParseStatus::DeadlineElapsed
                          : interruption == sf::Interruption::Cancelled
                              ? sf::ParseStatus::Cancelled
                              : sf::ParseStatus::ClockFailure;
      if (result.status != status || result.cause != 6671 ||
          fixture.facts.entries != 0 ||
          fixture.facts.report_uuid != sf::Uuid{} ||
          fixture.control.calls != ordinal)
        return false;
    }
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
      {"golden actual module tuple and matching process annotations",
       GoldenSupervisor},
      {"golden full desktop subject and generation", GoldenDesktop},
      {"process cannot complete partial module tuple",
       GlobalDoesNotCompleteModule},
      {"process contradiction", GlobalContradiction},
      {"two complete host modules rejected", DuplicateHostModules},
      {"duplicate owned key rejected", OwnedDuplicateKey},
      {"foreign module Unicode and host index", ForeignModuleAndUnicode},
      {"missing and spliced tuples rejected", MissingTupleAndSplice},
      {"inert expected data validated before source read", ExpectedValidation},
      {"full namespace epoch build UUID equality", ExactFullIdentity},
      {"128 directories and claimed plus one overflow",
       HeaderAndDirectoryBounds},
      {"missing and duplicate required streams", RequiredAndDuplicateStreams},
      {"versions descriptor ranges reserved and legacy size",
       StreamRangesVersionsAndReserved},
      {"1024 actual modules links and plus one", ModuleAndLinkLimits},
      {"module indexes alias and exact record sizes",
       LinkIdentityAndExactSizes},
      {"32 dictionary entries and claimed plus one", DictionaryEntryLimits},
      {"32 owned annotation observations and plus one", OwnedAnnotationBudget},
      {"64 byte keys 256 byte values and plus one", StringBoundaries},
      {"malformed UTF8 NUL and overflow string RVA", Utf8NulAndRvaBounds},
      {"256MiB retained file size and plus one without allocation",
       DumpSizeBound},
      {"truncated ranges never partial publication", TruncatedRanges},
      {"source failure exact cause no retry", SourceFailurePreservesCause},
      {"failed first middle final reads always observe post control and retain "
       "source cause",
       FailedReadStillObservesPostControl},
      {"original deadline cancel clock cause before after final reads",
       OriginalInterruptionPreservesCause},
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
