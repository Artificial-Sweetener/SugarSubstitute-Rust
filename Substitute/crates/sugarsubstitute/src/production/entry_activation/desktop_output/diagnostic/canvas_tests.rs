// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Canvas request lifecycle records cross the framed stderr relay under their own budget.
use super::*;
const LINE: &[u8] = b"2026-11-01T04:14:13.097161Z  INFO sugarsubstitute::canvas_surface_diagnostic: operation=\"canvas_surface_request\" schema=1 stage=Admitted op=Overview request_id=115 revision=19 generation=4 attachment=\"5321edfd\" pending=None wait=None intent=0 current=0 status=None\n";
const READINESS: &[u8] = b"INFO sugarsubstitute::production::startup_readiness_diagnostic: operation=\"startup_readiness_acknowledged\" schema=1 launch=1 navigation=2\n";

#[test]
fn canvas_records_survive_chunking_and_leave_the_shared_phase_cap_intact() {
    for split in 1..LINE.len() {
        let mut owner = Diagnostic::new();
        let mut seen = Vec::new();
        owner.feed(OutputStream::Stderr, 1, false, false, &LINE[..split], |v| seen.push(v));
        assert!(seen.is_empty());
        owner.feed(OutputStream::Stderr, 2, false, false, &LINE[split..], |v| seen.push(v));
        assert!(matches!(seen.as_slice(), [Observation::Canvas(record)]
            if record.request_id == 115 && record.revision == 19 && record.generation == 4));
    }
    let mut owner = Diagnostic::new();
    let mut canvas = 0_usize;
    let mut readiness = 0_usize;
    let mut sequence = 0;
    for _ in 0..usize::from(CANVAS_RECORDS) + 8 {
        sequence += 1;
        owner.feed(OutputStream::Stderr, sequence, false, false, LINE, |v| {
            canvas += usize::from(matches!(v, Observation::Canvas(_)));
        });
    }
    for _ in 0..40 {
        sequence += 1;
        owner.feed(OutputStream::Stderr, sequence, false, false, READINESS, |v| {
            readiness += usize::from(matches!(v, Observation::Readiness(_)));
        });
    }
    assert_eq!(canvas, usize::from(CANVAS_RECORDS));
    assert_eq!(readiness, 30);
}

#[test]
fn preparation_snapshot_crosses_chunked_parent_relay_under_the_canvas_budget() {
    const PREPARATION: &[u8] = b"INFO sugarsubstitute::canvas_preparation_diagnostic: operation=\"canvas_preparation_snapshot\" schema=1 request_id=25 intent=8 current=8 phase=Rendering flags=513 scene_source=8 buffers=0 render_id=42 render=Running display_id=0 display=None pacer=Ready\n";
    for split in 1..PREPARATION.len() {
        let mut owner = Diagnostic::new();
        let mut seen = Vec::new();
        owner.feed(OutputStream::Stderr, 1, false, false, &PREPARATION[..split], |v| seen.push(v));
        assert!(seen.is_empty());
        owner.feed(OutputStream::Stderr, 2, false, false, &PREPARATION[split..], |v| seen.push(v));
        assert!(
            matches!(seen.as_slice(), [Observation::CanvasPreparation(record)] if record.request_id == 25 && record.snapshot.render_id == 42)
        );
    }
    let mut owner = Diagnostic::new();
    let mut count = 0;
    for sequence in 1..=u64::from(CANVAS_RECORDS) + 1 {
        owner.feed(OutputStream::Stderr, sequence, false, false, PREPARATION, |_| count += 1);
    }
    owner.feed(OutputStream::Stderr, u64::from(CANVAS_RECORDS) + 2, false, false, LINE, |_| {
        count += 1
    });
    assert_eq!(count, CANVAS_RECORDS);
    let mut seen = Vec::new();
    owner.feed(OutputStream::Stderr, u64::from(CANVAS_RECORDS) + 3, false, false, READINESS, |v| {
        seen.push(v)
    });
    assert!(matches!(seen.as_slice(), [Observation::Readiness(_)]));
}

#[test]
fn zoom_records_cross_the_relay_under_their_own_budget() {
    const ZOOM: &[u8] = b"2026-11-22T20:00:00.000000Z  INFO sugarsubstitute::canvas_zoom_diagnostic: operation=\"canvas_zoom\" schema=1 gap_us=12000 duration_us=96000 snap=Animated\n";
    for split in 1..ZOOM.len() {
        let mut owner = Diagnostic::new();
        let mut seen = Vec::new();
        owner.feed(OutputStream::Stderr, 1, false, false, &ZOOM[..split], |v| seen.push(v));
        assert!(seen.is_empty());
        owner.feed(OutputStream::Stderr, 2, false, false, &ZOOM[split..], |v| seen.push(v));
        assert!(matches!(seen.as_slice(), [Observation::Zoom(record)]
            if record.gap == Some(12_000) && record.duration == 96_000));
    }
    let mut owner = Diagnostic::new();
    let (mut zoom, mut canvas) = (0_u16, 0_u16);
    let mut sequence = 0;
    for _ in 0..u32::from(ZOOM_RECORDS) + 8 {
        sequence += 1;
        owner.feed(OutputStream::Stderr, sequence, false, false, ZOOM, |v| {
            zoom += u16::from(matches!(v, Observation::Zoom(_)));
        });
    }
    sequence += 1;
    owner.feed(OutputStream::Stderr, sequence, false, false, LINE, |v| {
        canvas += u16::from(matches!(v, Observation::Canvas(_)));
    });
    assert_eq!((zoom, canvas), (ZOOM_RECORDS, 1));
}

#[test]
fn preview_timing_records_cross_the_relay_under_their_own_budget() {
    const PREVIEW: &[u8] = b"2026-11-22T20:00:00.000000Z  INFO sugarsubstitute::canvas_preview_diagnostic: operation=\"canvas_preview_offered\" schema=1 frame=9 load_us=120000 prepare_us=30000 offer_us=5000\n";
    for split in 1..PREVIEW.len() {
        let mut owner = Diagnostic::new();
        let mut seen = Vec::new();
        owner.feed(OutputStream::Stderr, 1, false, false, &PREVIEW[..split], |v| seen.push(v));
        assert!(seen.is_empty());
        owner.feed(OutputStream::Stderr, 2, false, false, &PREVIEW[split..], |v| seen.push(v));
        assert!(matches!(seen.as_slice(), [Observation::Preview(record)]
            if record.token == 9 && record.load == 120_000 && record.offer == 5_000));
    }
    let mut owner = Diagnostic::new();
    let mut count = 0_u16;
    for sequence in 1..=u64::from(PREVIEW_RECORDS) + 8 {
        owner.feed(OutputStream::Stderr, sequence, false, false, PREVIEW, |v| {
            count += u16::from(matches!(v, Observation::Preview(_)));
        });
    }
    assert_eq!(count, PREVIEW_RECORDS);
}

#[test]
fn selection_records_cross_the_relay_under_their_own_budget() {
    const SELECTION: &[u8] = b"2026-11-23T20:00:00.000000Z  INFO sugarsubstitute::canvas_selection_diagnostic: operation=\"canvas_selection_shown\" schema=1 selection=6 route=Retained tier=Detail load_us=0 shown_us=9000\n";
    for split in 1..SELECTION.len() {
        let mut owner = Diagnostic::new();
        let mut seen = Vec::new();
        owner.feed(OutputStream::Stderr, 1, false, false, &SELECTION[..split], |v| seen.push(v));
        assert!(seen.is_empty());
        owner.feed(OutputStream::Stderr, 2, false, false, &SELECTION[split..], |v| seen.push(v));
        assert!(matches!(seen.as_slice(), [Observation::Selection(record)]
            if record.selection == 6 && record.shown == 9_000
                && record.route == crate::canvas_selection_diagnostic::Route::Retained));
    }
    let mut owner = Diagnostic::new();
    let mut count = 0_u16;
    for sequence in 1..=u64::from(SELECTION_RECORDS) + 8 {
        owner.feed(OutputStream::Stderr, sequence, false, false, SELECTION, |v| {
            count += u16::from(matches!(v, Observation::Selection(_)));
        });
    }
    assert_eq!(count, SELECTION_RECORDS);
}

#[test]
fn motion_records_cross_the_relay_under_their_own_budget() {
    const MOTION: &[u8] = b"2026-11-23T05:00:00.000000Z  INFO sugarsubstitute::canvas_motion_diagnostic: operation=\"canvas_motion\" schema=1 elapsed_us=86000 period_us=6944 first_frame_us=4000 accepted=12 submitted=12 abandoned=0 skipped=0 longest_gap_us=7100 detail=false blocked=disp:0,gpu:0,cpu:0,pace:3,back:0,same:0,buf:0,ret:5,rest:0,acq:0,init:0,pick:0,ginit:0,route:0,open:0,susp:0,stale:0\n";
    for split in 1..MOTION.len() {
        let mut owner = Diagnostic::new();
        let mut seen = Vec::new();
        owner.feed(OutputStream::Stderr, 1, false, false, &MOTION[..split], |v| seen.push(v));
        assert!(seen.is_empty());
        owner.feed(OutputStream::Stderr, 2, false, false, &MOTION[split..], |v| seen.push(v));
        assert!(matches!(seen.as_slice(), [Observation::Motion(record)]
            if record.accepted == 12 && record.period == 6_944 && record.first_frame == Some(4_000)));
    }
    let mut owner = Diagnostic::new();
    let mut count = 0_u16;
    for sequence in 1..=u64::from(MOTION_RECORDS) + 8 {
        owner.feed(OutputStream::Stderr, sequence, false, false, MOTION, |v| {
            count += u16::from(matches!(v, Observation::Motion(_)));
        });
    }
    assert_eq!(count, MOTION_RECORDS);
}

#[test]
fn presentation_windows_cross_the_relay_under_their_own_budget() {
    const PRESENT: &[u8] = b"2026-11-24T05:00:00.000000Z  INFO sugarsubstitute_desktop::canvas_present_diagnostic: operation=\"canvas_present\" schema=2 window_us=1000250 presents=60 created=1 updated=12 wrapped=47 repaired=9 scrolled=4 unchanged=2 regions=14 source_bytes=431308800 fill_us=41000 draw_us=900 signal_us=310 commit_us=5200 slowest_us=3100 width=3840 height=2160\n";
    for split in 1..PRESENT.len() {
        let mut owner = Diagnostic::new();
        let mut seen = Vec::new();
        owner.feed(OutputStream::Stderr, 1, false, false, &PRESENT[..split], |v| seen.push(v));
        assert!(seen.is_empty());
        owner.feed(OutputStream::Stderr, 2, false, false, &PRESENT[split..], |v| seen.push(v));
        assert!(matches!(seen.as_slice(), [Observation::Present(record)]
            if record.presents == 60 && record.updated == 12 && record.fill_us == 41_000
                && record.signal_us == 310 && record.unchanged == 2 && record.width == 3_840));
    }
    let mut owner = Diagnostic::new();
    let mut count = 0_u16;
    for sequence in 1..=u64::from(PRESENT_RECORDS) + 8 {
        owner.feed(OutputStream::Stderr, sequence, false, false, PRESENT, |v| {
            count += u16::from(matches!(v, Observation::Present(_)));
        });
    }
    assert_eq!(count, PRESENT_RECORDS);
}

#[test]
fn gpu_phase_windows_cross_the_relay_under_their_own_budget() {
    const GPU: &[u8] = b"2026-11-25T10:00:00.000000Z  INFO sugarsubstitute_image_gpu::timing_diagnostic: operation=\"image_gpu_timing\" schema=1 phase=\"composition_reads\" vendor=4318 device=11141 window_us=1000400 completed=58 total_us=900 max_us=120 bytes=0 work_items=58 wait_timeouts=0 active=0 peak_active=1 busy_us=0\n";
    for split in 1..GPU.len() {
        let mut owner = Diagnostic::new();
        let mut seen = Vec::new();
        owner.feed(OutputStream::Stderr, 1, false, false, &GPU[..split], |v| seen.push(v));
        assert!(seen.is_empty());
        owner.feed(OutputStream::Stderr, 2, false, false, &GPU[split..], |v| seen.push(v));
        assert!(matches!(seen.as_slice(), [Observation::GpuTiming(record)]
            if record.phase == "composition_reads" && record.completed == 58
                && record.total_us == 900));
    }
    let mut owner = Diagnostic::new();
    let mut count = 0_u16;
    for sequence in 1..=u64::from(GPU_TIMING_RECORDS) + 8 {
        owner.feed(OutputStream::Stderr, sequence, false, false, GPU, |v| {
            count += u16::from(matches!(v, Observation::GpuTiming(_)));
        });
    }
    assert_eq!(count, GPU_TIMING_RECORDS);
}
