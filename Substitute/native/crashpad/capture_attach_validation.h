// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#ifndef SUGARSUBSTITUTE_CAPTURE_ATTACH_VALIDATION_H_
#define SUGARSUBSTITUTE_CAPTURE_ATTACH_VALIDATION_H_
#include "sugarsubstitute/capture_attach_v2.h"

namespace sugarsubstitute {
// Inert ABI validation precedes role admission; kernel identity is authenticated
// separately on the actual adopted connection and retained process handles.
bool ValidAttachInput(const SsCaptureAttachInputV2& input);
bool ValidAttachProgress(const SsCaptureAttachProgressV2& progress);
bool ValidAttachStep(const SsCaptureAttachStepV2& step);
}  // namespace sugarsubstitute
#endif
