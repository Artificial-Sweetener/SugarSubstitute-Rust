# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Versioned bounded display observations and their common coordinate contract."""
from __future__ import annotations

from dataclasses import dataclass


def integer(value, minimum, maximum, name):
    if type(value) is not int or not minimum <= value <= maximum:
        raise ValueError(f"{name} must be an integer in [{minimum}, {maximum}]")
    return value


@dataclass(frozen=True)
class Region:
    """A 2..16 physical-pixel rectangle relative to the client origin."""
    x: int
    y: int
    width: int
    height: int

    def __post_init__(self):
        integer(self.x, 0, 0x7fffffff, "region x")
        integer(self.y, 0, 0x7fffffff, "region y")
        integer(self.width, 2, 16, "region width")
        integer(self.height, 2, 16, "region height")


@dataclass(frozen=True)
class Geometry:
    """Physical client dimensions, native DPI and signed screen origin."""
    width: int
    height: int
    dpi: int
    screen_x: int
    screen_y: int


@dataclass(frozen=True)
class Interval:
    """Actual QPC stage; zero/zero means this native stage has not started."""
    begin: int
    end: int


@dataclass(frozen=True)
class GdiMetadata:
    flush_begin_ticks: int | None
    flush_end_ticks: int | None
    copy_begin_ticks: int
    copy_end_ticks: int
    read_end_ticks: int


@dataclass(frozen=True)
class DxgiOutput:
    ui_thread_id: int
    output_index: int
    qpc_frequency: int
    created_qpc: int
    monitor: int
    adapter_luid_low: int
    adapter_luid_high: int
    output_rect: tuple[int, int, int, int]
    roi: tuple[int, int, int, int]
    rotation: int
    color_space: int
    bits_per_color: int
    advanced_color_flags: int
    texture_format: int
    desktop_width: int
    desktop_height: int
    staging_width: int
    staging_height: int
    output_name: str
    adapter_description: str
    color_contract: str


@dataclass(frozen=True)
class DxgiMetadata:
    output: DxgiOutput
    pending_reason: str
    call: Interval
    sequence: int
    last_present_qpc: int
    last_mouse_qpc: int
    accumulated_frames: int
    protected_content_masked: int
    rects_coalesced: int
    pointer_visible: int
    pointer_known: int
    pointer_type: int
    pointer_x: int
    pointer_y: int
    pointer_width: int
    pointer_height: int
    pointer_pitch: int
    pointer_shape_bytes: int
    acquire: Interval
    copy_submit: Interval
    fence_wait: Interval
    map_read: Interval
    release_frame: Interval


@dataclass(frozen=True)
class CopyResult:
    backend: str
    bgra: bytes | None
    metadata: GdiMetadata | DxgiMetadata


@dataclass(frozen=True)
class RegionSample:
    """Schema2 ready pixels. Alpha is unspecified; end includes final guards.

    Native source timestamps establish desktop freshness, not product publication
    or an exact display transition. Never subtract sampling overhead.
    """
    bgra: bytes
    hwnd: int
    pid: int
    creation_ticks: int
    geometry: Geometry
    region: Region
    qpc_frequency: int
    begin_ticks: int
    ownership_before_end_ticks: int
    storage_begin_ticks: int
    storage_end_ticks: int
    end_ticks: int
    metadata: GdiMetadata | DxgiMetadata
    backend: str
    schema_version: int = 2
    kind: str = "ready"


@dataclass(frozen=True)
class RegionPending:
    """Schema2 pending attempt; contains no semantic pixel bytes."""
    hwnd: int
    pid: int
    creation_ticks: int
    geometry: Geometry
    region: Region
    qpc_frequency: int
    begin_ticks: int
    ownership_before_end_ticks: int
    storage_begin_ticks: int
    storage_end_ticks: int
    end_ticks: int
    metadata: DxgiMetadata
    backend: str
    schema_version: int = 2
    kind: str = "pending"


def validate_record(sample, *, frequency, deadline, minimum_present=0):
    """Validate the current explicit schema and final conservative deadline.

    Historical unversioned GDI artifacts remain historical bytes; this live
    contract does not infer a backend for unknown or old records.
    """
    integer(deadline, 1, 0x7fffffffffffffff, "capture deadline")
    integer(minimum_present, 0, 0x7fffffffffffffff, "source floor")
    if type(sample) not in (RegionSample, RegionPending) or sample.schema_version != 2:
        raise RuntimeError("Unknown region observation schema")
    if sample.qpc_frequency != frequency or type(sample.qpc_frequency) is not int:
        raise RuntimeError("ROI and input clocks differ")
    ticks = [sample.begin_ticks, sample.ownership_before_end_ticks, sample.storage_begin_ticks,
             sample.storage_end_ticks, sample.end_ticks]
    for tick in ticks:
        integer(tick, 1, 0x7fffffffffffffff, "capture tick")
    if ticks != sorted(ticks):
        raise RuntimeError("Capture timing brackets are invalid or reversed")
    if sample.end_ticks >= deadline:
        raise RuntimeError("Owned capture exceeded original absolute deadline")
    if type(sample) is RegionSample:
        if sample.kind != "ready" or type(sample.bgra) is not bytes or len(sample.bgra) != sample.region.width * sample.region.height * 4:
            raise RuntimeError("Ready capture lacks exact bounded region bytes")
    elif sample.kind != "pending" or sample.backend != "dxgi_v1":
        raise RuntimeError("Pending capture has an invalid backend or kind")
    if sample.backend == "gdi":
        if type(sample) is not RegionSample or type(sample.metadata) is not GdiMetadata:
            raise RuntimeError("GDI capture has an invalid stage contract")
        stages = sample.metadata
        ordered = [sample.ownership_before_end_ticks]
        if stages.flush_begin_ticks is not None:
            ordered.extend([stages.flush_begin_ticks, stages.flush_end_ticks])
        elif stages.flush_end_ticks is not None:
            raise RuntimeError("GDI flush timing lacks its beginning")
        ordered.extend([stages.copy_begin_ticks, stages.copy_end_ticks, stages.read_end_ticks, sample.storage_end_ticks])
        if any(type(t) is not int or t < 1 for t in ordered) or ordered != sorted(ordered):
            raise RuntimeError("GDI timing brackets are invalid or reversed")
    elif sample.backend == "dxgi_v1":
        if type(sample.metadata) is not DxgiMetadata:
            raise RuntimeError("DXGI capture lacks native metadata")
        call = sample.metadata.call
        if not sample.storage_begin_ticks <= call.begin <= call.end <= sample.storage_end_ticks:
            raise RuntimeError("DXGI call lies outside actual observer brackets")
        if type(sample) is RegionPending and sample.metadata.pending_reason not in (
                "acquire_timeout", "fence_pending", "map_pending", "pointer_only", "stale", "deadline"):
            raise RuntimeError("DXGI pending record has an invalid reason")
        if type(sample) is RegionSample and sample.metadata.pending_reason != "ready":
            raise RuntimeError("DXGI ready record has a pending reason")
        if type(sample) is RegionSample and (sample.metadata.last_present_qpc < minimum_present
                                            or not 0 < sample.metadata.last_present_qpc <= sample.end_ticks):
            raise RuntimeError("DXGI source is zero, stale or future")
    else:
        raise RuntimeError("Unknown capture backend")
    return sample
