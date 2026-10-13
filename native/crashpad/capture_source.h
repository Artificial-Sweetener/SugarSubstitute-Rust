// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#ifndef SUGARSUBSTITUTE_CAPTURE_SOURCE_H_
#define SUGARSUBSTITUTE_CAPTURE_SOURCE_H_
#include "sugarsubstitute/capture_endpoint.h"
#include "sugarsubstitute/owned_start.h"
#include <atomic>

namespace sugarsubstitute {
// A single publisher freezes matching facts after actual registration and
// checked temporary retirement. Readers perform no allocation or native call.
class CaptureSource {
 public:
  bool Publish(const OwnedStart& owner, const std::wstring& endpoint);
  void Revoke();
  bool Endpoint(const SsCaptureEndpointExpectationV1* expected,
                SsCaptureEndpointV1* output) const;
  bool Identity(SsCaptureSourceIdentityV1* output) const;
 private:
  SsCaptureEndpointV1 endpoint_ = {};
  SsCaptureSourceIdentityV1 identity_ = {};
  std::atomic<bool> available_{false};
  bool frozen_ = false;
};
}  // namespace sugarsubstitute
#endif
