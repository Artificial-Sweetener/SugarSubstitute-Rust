// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#include "format_acquisition.h"
#include "native_error.h"
#include "observer_fake.h"
#include "output_policy.h"
#include "pixel_rows.h"
#include <array>
#include <cstdio>
#include <cstring>
#include <d3d11_4.h>
#include <dxgi.h>
#include <dxgi1_6.h>
#include <stdexcept>
#include <string>
#include <wrl/client.h>

void run_native_tests();
namespace {
using namespace ss::dxgi;
using namespace ss::dxgi::tests;
void request_rejection() {
    auto r = request();
    check(validate_request(r) == S_OK, "Valid negative-origin ROI rejected");
    r.width = 1;
    check(validate_request(r) == E_INVALIDARG, "Unbounded/invalid ROI accepted");
    r = request();
    r.fence_timeout_ms = 6;
    check(validate_request(r) == E_INVALIDARG, "Unbounded native wait accepted");
    r = request();
    r.x = INT32_MAX;
    check(validate_request(r) == E_INVALIDARG, "Signed ROI overflow accepted");
    r = request();
    r.size--;
    check(validate_request(r) == E_INVALIDARG, "Wrong ABI size accepted");
}
void finite_success_and_exact_release() {
    Fake p;
    Owner owner(p, request(), info());
    ss_dxgi_sample s{};
    check(owner.sample(0, 1000, s) == S_OK, "Fresh source not observed");
    check(s.byte_count == 16 && s.bgra[0] == 0x71 && s.sequence == 1, "Exact bytes/sequence lost");
    check(s.acquire.begin <= s.acquire.end && s.release_frame.end <= s.call_end_qpc,
          "Stage ordering lost");
    check(p.acquire_timeout == 5 && p.fence_timeout == 5, "Finite configured waits lost");
    check(p.release_calls == 1 && !owner.error().frame_owned,
          "Frame lease not released exactly once");
    check(owner.close() == S_OK && p.release_calls == 1, "Close double-released frame");
}
void pending_resume_retains_source_and_never_reacquires() {
    Fake p;
    p.fence = S_FALSE;
    Owner owner(p, request(), info());
    ss_dxgi_sample s{};
    check(owner.sample(0, 1000, s) == S_FALSE && s.byte_count == 0, "Fence pending became success");
    const auto acquired = s.acquire;
    const auto source = s.last_present_qpc;
    check(owner.close() == S_FALSE && p.dispose_calls == 0 && owner.error().event_registered,
          "Pending close falsely stopped");
    // A separate owner resumes without first requesting close, which closes admission.
    Fake resumed;
    resumed.fence = S_FALSE;
    Owner second(resumed, request(), info());
    check(second.sample(0, 1000, s) == S_FALSE, "Expected second pending");
    resumed.fence = S_OK;
    resumed.map = DXGI_ERROR_WAS_STILL_DRAWING;
    check(second.sample(0, 1000, s) == S_FALSE && s.pending_reason == SS_DXGI_MAP_PENDING,
          "Blocking/false successful Map");
    resumed.map = S_OK;
    check(second.sample(0, 1000, s) == S_OK && resumed.acquire_calls == 1, "Held frame reacquired");
    check(s.last_present_qpc == source && s.acquire.begin == acquired.begin,
          "Held source changed across pending calls");
    check(second.close() == S_OK, "Resumed close failed");
    p.fence = S_OK;
    check(owner.close() == S_OK && p.release_calls == 1, "Final pending cleanup failed");
}
void timeout_pointer_only_stale_and_future() {
    for (const auto source : {int64_t{0}, int64_t{9}}) {
        Fake p;
        p.present = source;
        Owner owner(p, request(), info());
        ss_dxgi_sample s{};
        check(owner.sample(10, 1000, s) == S_FALSE && s.byte_count == 0,
              "Stale/pointer-only became feedback");
        check(p.release_calls == 1 && !p.pending, "Rejected source queued GPU work");
        check(owner.close() == S_OK, "Rejected source close failed");
    }
    Fake p;
    p.acquisition = DXGI_ERROR_WAIT_TIMEOUT;
    Owner owner(p, request(), info());
    ss_dxgi_sample s{};
    check(owner.sample(0, 1000, s) == S_FALSE && s.pending_reason == SS_DXGI_ACQUIRE_TIMEOUT,
          "Timeout not pending");
    check(p.release_calls == 0, "Timeout fabricated frame lease");
    check(owner.close() == S_OK, "Timeout cleanup failed");
    Fake future;
    future.present = 1001;
    Owner other(future, request(), info());
    check(FAILED(other.sample(0, 1000, s)), "Future source accepted");
    check(other.close() == S_OK && future.release_calls == 1, "Future source cleanup lost");
}
void counter_failure_keeps_actual_frame_and_primary_cleanup_causes() {
    Fake p;
    p.fail_now = 4;
    p.release = DXGI_ERROR_ACCESS_LOST;
    Owner owner(p, request(), info());
    ss_dxgi_sample s{};
    check(owner.sample(0, 1000, s) == E_FAIL && owner.error().frame_owned,
          "QPC failure erased acquired obligation");
    check(owner.close() == S_OK, "Completed poisoned close could not consume");
    const auto error = owner.error();
    check(error.hresult == E_FAIL && error.operation == SS_DXGI_QPC, "Original clock cause erased");
    check(error.cleanup_hresult == DXGI_ERROR_ACCESS_LOST && p.release_calls == 1,
          "Cleanup cause/exact-once release lost");
    check(owner.close() == S_OK && p.release_calls == 1, "Poison cleanup repeated release");
}
void partial_native_acquire_and_failed_gpu_retention() {
    Fake p;
    p.acquisition = E_ACCESSDENIED;
    p.partial_acquire = true;
    Owner owner(p, request(), info());
    ss_dxgi_sample s{};
    check(owner.sample(0, 1000, s) == E_ACCESSDENIED && owner.error().frame_owned,
          "Post-acquire rejection erased lease");
    check(owner.close() == S_OK && p.release_calls == 1, "Partial acquire not cleaned");
    Fake gpu;
    gpu.copy = E_FAIL;
    gpu.fence = E_FAIL;
    Owner held(gpu, request(), info());
    check(held.sample(0, 1000, s) == E_FAIL, "Failed GPU submit not poisoned");
    check(held.close() == E_FAIL && gpu.dispose_calls == 0 && held.error().gpu_pending,
          "Unknown GPU stop falsely consumed");
    gpu.fence = S_OK;
    check(held.close() == S_OK, "Proved GPU completion not cleaned");
}
void deadline_affinity_and_output_poison() {
    Fake p;
    Owner owner(p, request(), info());
    ss_dxgi_sample s{};
    check(owner.sample(0, 11, s) == S_FALSE && s.pending_reason == SS_DXGI_DEADLINE &&
              p.acquire_calls == 0,
          "Expired budget reached native acquire");
    p.current_thread = 8;
    check(owner.sample(0, 1000, s) == RPC_E_WRONG_THREAD && owner.close() == RPC_E_WRONG_THREAD,
          "Foreign thread accepted");
    p.current_thread = 7;
    p.output = E_ABORT;
    check(owner.sample(0, 1000, s) == E_ABORT && owner.error().poisoned,
          "Output change did not poison cohort");
    p.output = S_OK;
    check(owner.sample(0, 1000, s) == E_ABORT && p.acquire_calls == 0, "Poison silently recovered");
    check(owner.close() == S_OK, "Output-poison cleanup failed");
    Fake short_wait;
    Owner bounded(short_wait, request(), info());
    check(bounded.sample(0, 16, s) == S_FALSE && short_wait.acquire_timeout <= 5,
          "Remaining deadline not clamped");
    check(bounded.close() == S_OK, "Deadline-held cleanup failed");
}
void partial_construction_cleanup() {
    Fake p;
    p.construction = E_OUTOFMEMORY;
    p.disposal = E_ACCESSDENIED;
    Owner owner(p, request(), info());
    ss_dxgi_info output{};
    check(owner.initialize(output) == E_OUTOFMEMORY, "Native construction failure hidden");
    check(owner.close() == E_OUTOFMEMORY && p.dispose_calls == 1,
          "Failed partial cleanup falsely consumed");
    check(owner.error().cleanup_hresult == E_ACCESSDENIED, "Partial cleanup cause not preserved");
    p.disposal = S_OK;
    check(owner.close() == S_OK && p.dispose_calls == 2 && p.acquire_calls == 0,
          "Retained partial construction not retried");
}
void final_end_observation_does_not_issue_false_success_sequence() {
    Fake p;
    Owner owner(p, request(), info());
    ss_dxgi_sample sample{};
    check(owner.sample(0, 24, sample) == S_FALSE && sample.pending_reason == SS_DXGI_DEADLINE,
          "Final clock deadline not rejected");
    check(sample.byte_count == 0 && sample.sequence == 0 && p.release_calls == 1,
          "Deadline attempt issued ready sequence or lost cleanup");
    check(owner.sample(0, 1000, sample) == S_OK && sample.sequence == 1,
          "First actual successful-ready ordinal skipped");
    check(owner.close() == S_OK, "Ready sequence fixture cleanup failed");
    Fake failure;
    failure.fail_now = 15;
    Owner poisoned(failure, request(), info());
    check(poisoned.sample(0, 1000, sample) == E_FAIL && sample.sequence == 0 &&
              sample.byte_count == 0,
          "Failed final clock issued successful-ready ordinal");
    check(failure.release_calls == 1 && poisoned.close() == S_OK,
          "Failed final clock erased actual frame release");
}
void native_failure_precedes_failed_end_clock_with_actual_obligations() {
    struct Boundary {
        HRESULT Fake::*failure;
        int end_counter;
        uint32_t operation;
    };
    constexpr Boundary boundaries[]{{&Fake::acquisition, 4, SS_DXGI_ACQUIRE},
                                    {&Fake::copy, 6, SS_DXGI_COPY},
                                    {&Fake::fence, 9, SS_DXGI_FENCE},
                                    {&Fake::map, 12, SS_DXGI_MAP}};
    for (const auto boundary : boundaries) {
        Fake port;
        port.*boundary.failure = E_ACCESSDENIED;
        port.partial_acquire = true;
        port.current_operation = boundary.operation;
        port.fail_now = boundary.end_counter;
        Owner owner(port, request(), info());
        ss_dxgi_sample sample{};
        check(owner.sample(0, 1000, sample) == E_ACCESSDENIED,
              "End clock erased earlier native failure");
        const auto error = owner.error();
        check(error.hresult == E_ACCESSDENIED && error.operation == boundary.operation &&
                  error.cleanup_hresult == E_FAIL && error.cleanup_operation == SS_DXGI_QPC,
              "Original native/secondary clock causes lost");
        check(error.frame_owned && sample.byte_count == 0 && sample.sequence == 0,
              "Failed observation erased actual frame or fabricated ready bytes");
        port.fence = S_OK; // External proof of actual queued GPU settlement.
        check(owner.close() == S_OK && port.release_calls == 1 && !port.pending,
              "Native/clock failure did not retain exact cleanup obligations");
    }
}
void expected_pending_results_do_not_replace_clock_failure() {
    for (const auto map_pending : {false, true}) {
        Fake port;
        if (map_pending)
            port.map = DXGI_ERROR_WAS_STILL_DRAWING;
        else
            port.acquisition = DXGI_ERROR_WAIT_TIMEOUT;
        port.fail_now = map_pending ? 12 : 4;
        Owner owner(port, request(), info());
        ss_dxgi_sample sample{};
        check(owner.sample(0, 1000, sample) == E_FAIL && owner.error().operation == SS_DXGI_QPC &&
                  SUCCEEDED(owner.error().cleanup_hresult),
              "Expected pending result was recorded as unexpected native failure");
        check(owner.close() == S_OK && port.release_calls == (map_pending ? 1 : 0),
              "Pending/clock cleanup ownership was fabricated or lost");
    }
}
void zero_clocks_and_unset_native_errors_remain_failures() {
    check(win32_failure(0) == E_FAIL && win32_failure(ERROR_ACCESS_DENIED) == E_ACCESSDENIED,
          "Failed BOOL with unset LastError became success or erased native cause");
    Fake creation;
    creation.created = 0;
    Owner owner(creation, request(), info());
    ss_dxgi_info observed{};
    check(owner.initialize(observed) == E_UNEXPECTED && owner.error().operation == SS_DXGI_QPC,
          "Zero creation counter accepted");
    check(owner.close() == S_OK && creation.acquire_calls == 0,
          "Rejected creation reached capture or lost resource cleanup");
    Fake sample_clock;
    sample_clock.ticks = 0;
    Owner zero(sample_clock, request(), info());
    ss_dxgi_sample sample{};
    check(zero.sample(0, 1000, sample) == E_UNEXPECTED && sample_clock.acquire_calls == 0 &&
              sample.byte_count == 0 && zero.close() == S_OK,
          "Zero sample clock accepted");
}
void diagnostic_reacquisition_requires_observed_settlement() {
    FormatAcquisition timeout;
    check(!format_acquisition_settled(timeout), "Unattempted acquisition permitted retry");
    timeout.attempted = true;
    timeout.acquire = DXGI_ERROR_WAIT_TIMEOUT;
    check(format_acquisition_settled(timeout), "Observed timeout prevented transition");
    for (const auto failure : {E_FAIL, S_FALSE}) {
        auto clock = timeout;
        clock.clock = failure;
        check(!format_acquisition_settled(clock), "Uncertain clock permitted reacquisition");
        auto acquire = timeout;
        acquire.acquire = failure;
        check(!format_acquisition_settled(acquire), "Unexpected acquisition permitted retry");
    }
    auto frame = timeout;
    frame.acquire = S_OK;
    frame.acquired = true;
    frame.query = frame.release = S_OK;
    check(format_acquisition_settled(frame), "Released queryable frame prevented transition");
    for (const auto failure : {E_FAIL, S_FALSE}) {
        auto query = frame;
        query.query = failure;
        check(!format_acquisition_settled(query), "Uncertain metadata permitted retry");
        auto release = frame;
        release.release = failure;
        check(!format_acquisition_settled(release), "Uncertain release permitted reacquisition");
    }
    frame.acquired = false;
    check(!format_acquisition_settled(frame), "Contradictory lease permitted reacquisition");
    timeout.acquired = true;
    check(!format_acquisition_settled(timeout), "Timeout with owned frame permitted retry");
}
} // namespace
int main() {
    try {
        request_rejection();
        finite_success_and_exact_release();
        pending_resume_retains_source_and_never_reacquires();
        timeout_pointer_only_stale_and_future();
        counter_failure_keeps_actual_frame_and_primary_cleanup_causes();
        partial_native_acquire_and_failed_gpu_retention();
        deadline_affinity_and_output_poison();
        partial_construction_cleanup();
        final_end_observation_does_not_issue_false_success_sequence();
        native_failure_precedes_failed_end_clock_with_actual_obligations();
        expected_pending_results_do_not_replace_clock_failure();
        zero_clocks_and_unset_native_errors_remain_failures();
        diagnostic_reacquisition_requires_observed_settlement();
        run_native_tests();
        std::puts("17 DXGI tests passed (16 owner/native + 1 diagnostic; no capture/GUI)");
        return 0;
    } catch (const std::exception &error) {
        std::fprintf(stderr, "DXGI owner test failed: %s\n", error.what());
        return 1;
    }
}
