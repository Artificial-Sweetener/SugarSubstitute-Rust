// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
#define SS_DXGI_BUILD
#include "observer.h"
#include <new>

struct ss_dxgi_context {
    std::unique_ptr<ss::dxgi::Port> port;
    ss::dxgi::Owner owner;
    ss_dxgi_context(std::unique_ptr<ss::dxgi::Port> p, const ss_dxgi_request &r,
                    const ss_dxgi_info &i)
        : port(std::move(p)), owner(*port, r, i) {}
};
namespace {
template <class T> bool valid(const T *p) noexcept {
    return p && p->version == SS_DXGI_VERSION && p->size == sizeof(T);
}
int32_t argument_error(ss_dxgi_error *error) noexcept {
    if (valid(error)) {
        *error = {};
        error->version = SS_DXGI_VERSION;
        error->size = sizeof(*error);
        error->hresult = E_INVALIDARG;
        error->operation = SS_DXGI_ARGUMENT;
    }
    return E_INVALIDARG;
}
} // namespace
extern "C" int32_t __cdecl ss_dxgi_v1_create(const ss_dxgi_request *request,
                                             ss_dxgi_context **context, ss_dxgi_info *info,
                                             ss_dxgi_error *error) {
    if (!valid(request) || !context || *context || !valid(info) || !valid(error))
        return argument_error(error);
    *error = {};
    error->version = SS_DXGI_VERSION;
    error->size = sizeof(*error);
    auto hr = ss::dxgi::validate_request(*request);
    if (FAILED(hr))
        return argument_error(error);
    try {
        auto port = ss::dxgi::native_port();
        ss_dxgi_info initial{};
        initial.ui_thread_id = port->thread();
        auto owned = std::make_unique<ss_dxgi_context>(std::move(port), *request, initial);
        hr = owned->owner.initialize(*info);
        if (FAILED(hr)) {
            const auto cleanup = owned->owner.close();
            *error = owned->owner.error();
            if (cleanup != S_OK)
                *context = owned.release();
            return hr;
        }
        *context = owned.release();
        return S_OK;
    } catch (const std::bad_alloc &) {
        error->hresult = E_OUTOFMEMORY;
        error->operation = SS_DXGI_DEVICE;
        return E_OUTOFMEMORY;
    } catch (...) {
        error->hresult = E_UNEXPECTED;
        error->operation = SS_DXGI_DEVICE;
        return E_UNEXPECTED;
    }
}
extern "C" int32_t __cdecl ss_dxgi_v1_try_sample(ss_dxgi_context *context, int64_t minimum,
                                                 int64_t deadline, ss_dxgi_sample *sample,
                                                 ss_dxgi_error *error) {
    if (!context || !valid(sample) || !valid(error))
        return argument_error(error);
    const auto hr = context->owner.sample(minimum, deadline, *sample);
    *error = context->owner.error();
    if (FAILED(hr) && SUCCEEDED(error->hresult)) {
        error->hresult = hr;
        error->operation = hr == RPC_E_WRONG_THREAD ? SS_DXGI_THREAD : SS_DXGI_ARGUMENT;
    }
    return hr;
}
extern "C" int32_t __cdecl ss_dxgi_v1_close(ss_dxgi_context **context, ss_dxgi_error *error) {
    if (!context || !valid(error))
        return argument_error(error);
    *error = {};
    error->version = SS_DXGI_VERSION;
    error->size = sizeof(*error);
    if (!*context)
        return S_OK;
    const auto hr = (*context)->owner.close();
    *error = (*context)->owner.error();
    if (FAILED(hr) && SUCCEEDED(error->hresult)) {
        error->hresult = hr;
        error->operation = SS_DXGI_THREAD;
    }
    if (hr == S_OK) {
        delete *context;
        *context = nullptr;
    }
    return hr;
}
