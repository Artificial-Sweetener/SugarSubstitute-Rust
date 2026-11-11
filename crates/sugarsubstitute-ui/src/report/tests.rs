// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove each kind's commands, expansion motion, details virtualization and painted layout.
use super::*;
use crate::{
    accent::AccentPalette,
    form,
    scene::{Element, SceneResources},
    text::PaintSpec,
};
use std::{error::Error, time::Duration};
use sugarsubstitute_contracts::{
    configuration::Theme,
    execution::{Generation, MemoryLease, MemoryRelease, SurfaceId},
    presentation::{FrameIdentity, PreparedFrame},
};
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

struct NoRelease;
impl MemoryRelease for NoRelease {
    fn release(&self, _: u64) {}
}

const LABELS: ReportLabels = ReportLabels {
    close_window: "Close window",
    show_report: "Show report",
    hide_report: "Hide report",
    copy: "Copy report",
    export: "Export report",
    close: "Close",
    restart: "Close and request restart",
    cancel: "Cancel this launch",
    include_dump: "Include native memory dump",
    dump_warning: "Native memory dumps can contain passwords and private information.",
};

fn content(kind: ReportKind) -> ReportContent {
    ReportContent {
        kind,
        severity: ReportSeverity::Error,
        window_title: "SugarSubstitute".into(),
        title: "Startup failed".into(),
        message: "SugarSubstitute could not start. You can copy this report.".into(),
        summary: vec![SummaryRow { label: "Version".into(), value: "0.4.0".into() }],
        has_details: true,
    }
}

fn surface(height: f32, theme: Theme) -> PaintSpec {
    PaintSpec { theme, scale: 1.0, width: WIDTH as u32, height: height.ceil() as u32 }
}

fn report_text(lines: usize) -> String {
    (0..lines).map(|line| format!("cause {line}: the backend did not answer\n")).collect()
}

fn layout(
    kind: ReportKind,
    state: &ReportState,
    resources: &mut SceneResources,
) -> Result<ReportLayout> {
    Ok(ReportLayout::measure(
        &content(kind),
        &LABELS,
        state,
        surface(900.0, Theme::Dark),
        resources,
        None,
        4000.0,
    )?)
}

fn paint(
    frame: ReportFrame,
    spec: PaintSpec,
    resources: &mut SceneResources,
) -> Result<PreparedFrame> {
    let shape = sugarsubstitute_contracts::presentation::FrameShape::new(
        spec.width,
        spec.height,
        spec.scale.into(),
    )?;
    let lease =
        MemoryLease::new(crate::scene::FramePaint::reservation_bytes(shape), Box::new(NoRelease));
    let identity = FrameIdentity {
        surface: SurfaceId::new(1)?,
        state_generation: Generation::new(1)?,
        sequence: Generation::new(1)?,
    };
    let mut painting = frame.into_paint(spec, AccentPalette::PRODUCT, identity, lease)?;
    while !painting.step(resources)? {}
    Ok(painting.finish()?)
}

fn visible(frame: &ReportFrame) -> Vec<ReportAction> {
    frame
        .controls
        .iter()
        .filter(|control| control.availability.visible)
        .filter_map(|control| ReportAction::from_id(control.id))
        .collect()
}

#[test]
fn each_kind_offers_exactly_its_commands() {
    use ReportAction::*;
    assert_eq!(ReportKind::Crash.commands(), &[Copy, Export, Close, Restart]);
    assert_eq!(ReportKind::StartupFailure.commands(), &[Copy, Close]);
    assert_eq!(ReportKind::UnresponsiveCopy.commands(), &[Cancel, Close]);
    assert_eq!(ReportKind::UpdateFailure.commands(), &[Copy, Export, Close]);
    assert!(!ReportKind::UpdateFailure.commands().contains(&Restart));
    assert_eq!(ReportKind::UnresponsiveCopy.dismissal(), Cancel);
    assert_eq!(ReportKind::UpdateFailure.dismissal(), Close);
    assert!(ReportKind::Crash.offers_dump());
    assert!(!ReportKind::UpdateFailure.offers_dump());
}

#[test]
fn identities_are_stable_unique_and_skip_the_console_peer() {
    let mut ids: Vec<u16> = ReportAction::ALL.iter().map(|action| action.id().get()).collect();
    for action in ReportAction::ALL {
        assert_eq!(ReportAction::from_id(action.id()), Some(action));
    }
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), ReportAction::ALL.len());
    assert!(!ids.contains(&4));
}

#[test]
fn restart_appears_only_when_offered_and_becomes_primary() {
    let mut crash = ReportState::new(ReportKind::Crash);
    assert!(!crash.command(ReportAction::Restart).visible);
    assert_eq!(crash.primary(), ReportAction::Close);
    crash.set(ReportAction::Restart, CommandState { visible: true, enabled: true });
    assert_eq!(crash.primary(), ReportAction::Restart);
    let unresponsive = ReportState::new(ReportKind::UnresponsiveCopy);
    assert!(unresponsive.command(ReportAction::Cancel).visible);
    assert!(unresponsive.command(ReportAction::Close).visible);
    assert_eq!(unresponsive.primary(), ReportAction::Close, "Close it and start is primary");
}

#[test]
fn inventory_is_valid_and_lists_only_the_kinds_peers() -> Result {
    for kind in [
        ReportKind::Crash,
        ReportKind::StartupFailure,
        ReportKind::UnresponsiveCopy,
        ReportKind::UpdateFailure,
    ] {
        let inventory = ReportLayout::inventory(kind, &LABELS);
        form::validate(&inventory, 1.0)?;
        assert_eq!(inventory.len(), 2 + usize::from(kind.offers_dump()) + kind.commands().len());
        assert!(inventory.iter().all(|control| !control.availability.visible));
    }
    Ok(())
}

#[test]
fn expansion_eases_deterministically_over_its_duration() {
    let start = Duration::from_secs(3);
    let open = DetailsMotion::start(0.0, true, start);
    assert_eq!(open.sample(start), (0.0, false));
    let (half, done) = open.sample(start + EXPAND_DURATION / 2);
    assert!(!done && half > 0.5, "ease out reveals most of the text early: {half}");
    assert_eq!(open.sample(start + EXPAND_DURATION), (1.0, true));
    assert_eq!(open.sample(start + EXPAND_DURATION * 4), (1.0, true));
    let close = DetailsMotion::start(1.0, false, start);
    let (half, _) = close.sample(start + EXPAND_DURATION / 2);
    assert!(half > 0.5, "ease in keeps the text while collapse starts: {half}");
    assert_eq!(close.sample(start + EXPAND_DURATION).0, 0.0);
    let reverse = DetailsMotion::start(0.4, false, start);
    assert_eq!(reverse.sample(start).0, 0.4);
}

#[test]
fn details_text_splits_lines_measures_in_bounded_slices_and_virtualizes() -> Result {
    let mut resources = SceneResources::new("en")?;
    let long = "x".repeat(5000);
    let details = DetailsText::new(&format!("a\r\nb\0\n\n{long}\n"))?;
    assert_eq!(details.text(), format!("a\nb\u{fffd}\n\n{long}\n"));
    assert_eq!(details.line_count(), 6, "a, b, empty, and the long line in three parts");
    assert!(DetailsText::new(&"y".repeat(detailsmax() + 1)).is_err());

    let mut text = DetailsText::new(&report_text(400))?;
    let spec = surface(600.0, Theme::Dark);
    let width = ReportLayout::details_text_width();
    let mut slices = 0;
    while !text.measure(width, spec, &mut resources, None, 64)? {
        slices += 1;
        assert!(!text.measured(width, spec));
    }
    assert_eq!(slices, 6, "400 lines in slices of 64");
    assert!(text.measured(width, spec));
    assert!((text.height() - 400.0 * DetailsText::LINE_HEIGHT).abs() < 0.5);
    let shown: Vec<_> = text.visible(1900.0, 2160.0).collect();
    assert!(shown.len() <= 16, "only the visible band is painted");
    assert!(shown[0].0.starts_with("cause 100:"));
    assert!(
        !text.measure(width + 10.0, spec, &mut resources, None, 1)?,
        "a new width restarts measurement"
    );
    Ok(())
}

fn detailsmax() -> usize {
    super::details::MAX_BYTES
}

#[test]
fn footer_shows_each_kinds_commands() -> Result {
    let mut resources = SceneResources::new("en")?;
    for (kind, expected) in [
        (ReportKind::StartupFailure, vec![ReportAction::Copy, ReportAction::Close]),
        (
            ReportKind::UpdateFailure,
            vec![ReportAction::Copy, ReportAction::Export, ReportAction::Close],
        ),
        (ReportKind::Crash, vec![ReportAction::Copy, ReportAction::Export, ReportAction::Close]),
        (ReportKind::UnresponsiveCopy, vec![ReportAction::Cancel, ReportAction::Close]),
    ] {
        let state = ReportState::new(kind);
        let measured = layout(kind, &state, &mut resources)?;
        let frame = measured.compose(
            &content(kind),
            &LABELS,
            &state,
            surface(measured.height(0.0), Theme::Dark),
            None,
        )?;
        let footer: Vec<_> =
            visible(&frame).into_iter().filter(|a| kind.commands().contains(a)).collect();
        let mut sorted = expected.clone();
        sorted.reverse();
        assert_eq!(footer, sorted, "{kind:?} lays commands out right to left");
        assert!(visible(&frame).contains(&ReportAction::Dismiss));
        assert_eq!(visible(&frame).contains(&ReportAction::IncludeDump), kind.offers_dump());
    }
    Ok(())
}

#[test]
fn collapsed_and_expanded_layouts_differ_by_the_details_panel_and_paint_it() -> Result {
    let mut resources = SceneResources::new("en")?;
    let kind = ReportKind::StartupFailure;
    let mut state = ReportState::new(kind);
    let measured = layout(kind, &state, &mut resources)?;
    let (collapsed, expanded) = (measured.height(0.0), measured.height(1.0));
    assert!((expanded - collapsed - 272.0).abs() < 0.01, "spacing plus 260 px of report text");
    assert!(measured.height(0.5) > collapsed && measured.height(0.5) < expanded);

    let text = report_text(40);
    let mut details = DetailsText::new(&text)?;
    let spec = surface(expanded, Theme::Dark);
    while !details.measure(ReportLayout::details_text_width(), spec, &mut resources, None, 64)? {}

    let closed = measured.compose(
        &content(kind),
        &LABELS,
        &state,
        surface(collapsed, Theme::Dark),
        Some(&details),
    )?;
    assert!(closed.page.is_empty(), "collapsed details paint nothing");
    let toggle = closed
        .controls
        .iter()
        .find(|c| c.id == ReportAction::ToggleDetails.id())
        .ok_or("toggle")?;
    assert_eq!(toggle.label, "Show report");

    state.expanded = true;
    state.expansion = 1.0;
    let open = measured.compose(&content(kind), &LABELS, &state, spec, Some(&details))?;
    let toggle =
        open.controls.iter().find(|c| c.id == ReportAction::ToggleDetails.id()).ok_or("toggle")?;
    assert_eq!(toggle.label, "Hide report");
    assert!((open.viewport.height - 258.0).abs() < 0.01);
    assert!(open.content_height > open.viewport.height, "forty lines scroll");
    assert!(open.page.len() >= 13 && open.page.len() <= 15);
    assert!(matches!(&open.page[0], Element::Label { text, .. } if text.starts_with("cause 0:")));

    state.scroll = 1.0e6;
    let scrolled = measured.compose(&content(kind), &LABELS, &state, spec, Some(&details))?;
    assert!((scrolled.scroll - (scrolled.content_height - scrolled.viewport.height)).abs() < 0.01);
    assert!(
        matches!(scrolled.page.last(), Some(Element::Label { text, .. }) if text.starts_with("cause 39:"))
    );

    let viewport = open.viewport;
    let pixels = paint(open, spec, &mut resources)?;
    let at = |x: f32, y: f32| -> [u8; 4] {
        let index = ((y as usize) * spec.width as usize + x as usize) * 4;
        let p = &pixels.pixels()[index..index + 4];
        [p[0], p[1], p[2], p[3]]
    };
    let glyph = at(24.0 + 16.0, 56.0 + 10.0);
    assert!(glyph[0] > 150 && glyph[1] < 80, "error glyph keeps its authored red: {glyph:?}");
    let footer = at(4.0, expanded - 4.0);
    assert_eq!(footer, [32, 32, 32, 255], "dark footer band");
    let text_row = (viewport.y + 8.0) as usize..(viewport.y + 8.0 + 19.0) as usize;
    let ink = text_row
        .flat_map(|y| (40..200).map(move |x| (x, y)))
        .filter(|(x, y)| at(*x as f32, *y as f32)[0] > 160)
        .count();
    assert!(ink > 20, "first report line is painted inside the panel");
    let collapsed_pixels = paint(closed, surface(collapsed, Theme::Dark), &mut resources)?;
    assert_eq!(collapsed_pixels.shape().height(), collapsed.ceil() as u32);
    Ok(())
}

#[test]
fn light_theme_paints_the_light_footer_and_a_dark_close_glyph() -> Result {
    let mut resources = SceneResources::new("en")?;
    let kind = ReportKind::UpdateFailure;
    let state = ReportState::new(kind);
    let measured = layout(kind, &state, &mut resources)?;
    let spec = surface(measured.height(0.0), Theme::Light);
    let frame = measured.compose(&content(kind), &LABELS, &state, spec, None)?;
    let pixels = paint(frame, spec, &mut resources)?;
    let index = |x: usize, y: usize| (y * spec.width as usize + x) * 4;
    let footer = &pixels.pixels()[index(4, spec.height as usize - 4)..][..4];
    assert_eq!(footer, &[243, 243, 243, 255]);
    let ink = (11..21)
        .flat_map(|y| (WIDTH as usize - 28..WIDTH as usize - 18).map(move |x| (x, y)))
        .filter(|(x, y)| pixels.pixels()[index(*x, *y) + 3] > 100)
        .count();
    assert!(ink > 6, "close glyph uses the light theme's text tone");
    Ok(())
}

/// A 1366x768 laptop at 150% is 512 logical pixels tall; with a taskbar its work area is about
/// 480, and the report keeps 48 of that free.
const SMALL_SCREEN: f32 = 432.0;

fn long_message(kind: ReportKind) -> ReportContent {
    ReportContent {
        message: "The backend stopped while SugarSubstitute was starting, and every retry failed \
            with the same answer from the connection. "
            .repeat(30),
        ..content(kind)
    }
}

fn body_peer(frame: &ReportFrame, action: ReportAction) -> Option<&crate::form::Control> {
    frame.page_controls.iter().find(|control| control.id == action.id())
}

#[test]
fn a_short_window_keeps_the_title_bar_and_footer_and_scrolls_the_body() -> Result {
    let mut resources = SceneResources::new("en")?;
    let kind = ReportKind::Crash;
    let mut state = ReportState::new(kind);
    state.set(ReportAction::Restart, CommandState { visible: true, enabled: true });
    let measured = ReportLayout::measure(
        &long_message(kind),
        &LABELS,
        &state,
        surface(900.0, Theme::Dark),
        &mut resources,
        None,
        SMALL_SCREEN,
    )?;
    assert!(measured.height(0.0) > SMALL_SCREEN, "the long message does not fit");
    let spec = surface(SMALL_SCREEN, Theme::Dark);
    let frame = measured.compose(&long_message(kind), &LABELS, &state, spec, None)?;
    assert_eq!(frame.viewport.y, 32.0, "the body starts under the title bar");
    assert!((frame.viewport.y + frame.viewport.height - (SMALL_SCREEN - 68.0)).abs() < 0.01);
    assert!(frame.content_height > frame.viewport.height, "the body scrolls");
    let footer: Vec<_> = frame
        .controls
        .iter()
        .filter(|c| c.availability.visible && c.bounds.y > SMALL_SCREEN - 68.0)
        .filter_map(|c| ReportAction::from_id(c.id))
        .collect();
    for action in
        [ReportAction::Copy, ReportAction::Export, ReportAction::Close, ReportAction::Restart]
    {
        assert!(footer.contains(&action), "{action:?} stays in the footer");
    }
    assert!(frame.controls.iter().any(|c| c.id == ReportAction::Dismiss.id()));
    let toggle = body_peer(&frame, ReportAction::ToggleDetails).ok_or("toggle")?;
    assert!(!toggle.availability.visible, "the toggle is below the first screen of message");

    state.scroll = 1.0e6;
    let end = measured.compose(&long_message(kind), &LABELS, &state, spec, None)?;
    assert!((end.scroll - (end.content_height - end.viewport.height)).abs() < 0.01);
    let toggle = body_peer(&end, ReportAction::ToggleDetails).ok_or("toggle")?;
    assert!(toggle.availability.visible, "scrolling reaches the toggle");
    assert!(
        toggle.bounds.y >= 0.0 && toggle.bounds.y + toggle.bounds.height <= end.viewport.height
    );
    assert!(body_peer(&end, ReportAction::IncludeDump).is_some_and(|c| c.availability.visible));
    let pixels = paint(end, spec, &mut resources)?;
    assert_eq!(pixels.shape().height(), SMALL_SCREEN as u32);
    Ok(())
}

#[test]
fn small_screen_details_scroll_with_the_body_and_paint_only_visible_lines() -> Result {
    let mut resources = SceneResources::new("en")?;
    let kind = ReportKind::StartupFailure;
    let mut state = ReportState::new(kind);
    let measured = ReportLayout::measure(
        &long_message(kind),
        &LABELS,
        &state,
        surface(900.0, Theme::Dark),
        &mut resources,
        None,
        SMALL_SCREEN,
    )?;
    let spec = surface(SMALL_SCREEN, Theme::Dark);
    let mut details = DetailsText::new(&report_text(2000))?;
    while !details.measure(ReportLayout::details_text_width(), spec, &mut resources, None, 256)? {}
    state.expanded = true;
    state.expansion = 1.0;
    state.scroll = 20_000.0;
    let frame = measured.compose(&long_message(kind), &LABELS, &state, spec, Some(&details))?;
    assert!(frame.content_height > 2000.0 * DetailsText::LINE_HEIGHT, "all text is reachable");
    let lines: Vec<_> = frame
        .page
        .iter()
        .filter_map(|e| match e {
            Element::Label { text, .. } if text.starts_with("cause ") => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(!lines.is_empty() && lines.len() < 40, "{} lines painted", lines.len());
    assert!(frame.page.len() <= 256);
    let painted = paint(frame, spec, &mut resources)?;
    assert_eq!(painted.shape().height(), SMALL_SCREEN as u32);
    Ok(())
}

#[test]
fn the_fully_expanded_report_uses_the_compact_panel_on_a_small_screen() -> Result {
    let mut resources = SceneResources::new("en")?;
    let kind = ReportKind::StartupFailure;
    let state = ReportState::new(kind);
    let compact = ReportLayout::measure(
        &content(kind),
        &LABELS,
        &state,
        surface(900.0, Theme::Dark),
        &mut resources,
        None,
        500.0,
    )?;
    assert!((compact.height(1.0) - compact.height(0.0) - 172.0).abs() < 0.01);
    Ok(())
}
