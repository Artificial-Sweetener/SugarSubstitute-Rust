// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Every caption state from fake application facts, in English and in another language.
use super::*;
use std::time::Duration;
use sugarsubstitute_application::workspace_session::close_save::{
    CloseReason, CloseSaveOutcome, SaveVerdict, UnsavedCause,
};
use sugarsubstitute_contracts::{
    execution::Moment,
    localization::messages,
    update::{
        attempt::{AttemptOutcome, AttemptPhase, UpdateReport},
        status::RecordedUpdates,
    },
};

fn ready() -> UpdateStatus {
    let mut status = UpdateStatus::default();
    status.reconcile(RecordedUpdates { prepared: Some("2.0.0".into()), ..Default::default() });
    status
}

fn failed() -> UpdateStatus {
    let mut status = UpdateStatus::default();
    status.begin();
    let report = UpdateReport {
        version: Some("2.0.0".into()),
        phase: AttemptPhase::Downloading,
        causes: vec!["The connection was reset".into()],
    };
    status.finish(Some(AttemptOutcome::Failed(report)), 10);
    status
}

fn saving() -> CloseSaveProgress {
    CloseSaveProgress::Saving {
        reason: CloseReason::Close,
        started: Moment::from_nanos(1),
        cutoff: Moment::from_nanos(2),
    }
}

fn feed(status: Option<&UpdateStatus>) -> UpdateFeed {
    UpdateFeed::read(status, None, WorkflowActivity::Idle)
}

fn shown(view: &UpdateCaptionView) -> Vec<u16> {
    (UPDATE..=DISMISS).filter(|id| view.peer(*id).is_some_and(|peer| peer.visible)).collect()
}

#[test]
fn the_update_button_is_hidden_unless_an_update_is_ready_or_failed() {
    let en = messages("en");
    let mut caption = CaptionUpdates::default();
    assert!(!caption.observe(feed(None)));
    assert!(shown(&caption.view(en)).is_empty());
    let mut current = UpdateStatus::default();
    current.begin();
    current.finish(Some(AttemptOutcome::Current), 5);
    assert!(!caption.observe(feed(Some(&current))));
    assert!(shown(&caption.view(en)).is_empty());

    assert!(caption.observe(feed(Some(&ready()))));
    let view = caption.view(en);
    assert_eq!((view.glyph, shown(&view)), (UpdateGlyph::Ready, vec![UPDATE]));
    assert_eq!(view.update.text, "Update ready");

    assert!(caption.observe(feed(Some(&failed()))));
    let view = caption.view(en);
    assert_eq!((view.glyph, shown(&view)), (UpdateGlyph::Warning, vec![UPDATE]));
    assert_eq!(view.update.label, "Couldn't update. Show the report");
}

#[test]
fn a_ready_update_outranks_a_later_failed_check() {
    let mut status = ready();
    status.begin();
    status.finish(None, 20);
    assert!(status.failure().is_some());
    assert_eq!(feed(Some(&status)).glyph, UpdateGlyph::Ready);
}

#[test]
fn ready_offers_the_restart_and_the_offer_waits_for_a_running_workflow() {
    let en = messages("en");
    let mut caption = CaptionUpdates::default();
    caption.observe(feed(Some(&ready())));
    assert_eq!(caption.activate(UPDATE), (true, None));
    let view = caption.view(en);
    assert_eq!(shown(&view), vec![UPDATE, OFFER]);
    assert_eq!(view.offer.text, "Restart to apply");

    caption.observe(UpdateFeed::read(Some(&ready()), None, WorkflowActivity::Running));
    assert_eq!(caption.view(en).offer.text, "Restart after this finishes");
    assert_eq!(caption.view(messages("es")).offer.text, "Reiniciar al terminar");

    assert_eq!(caption.activate(OFFER), (true, Some(UpdateIntent::RestartToApply)));
    assert_eq!(shown(&caption.view(en)), vec![UPDATE]);
    // The offer is gone, so a second queued click does nothing.
    assert_eq!(caption.activate(OFFER), (false, None));
    // Clicking the button again toggles the offer closed.
    caption.activate(UPDATE);
    assert_eq!(caption.activate(UPDATE), (true, None));
    assert_eq!(shown(&caption.view(en)), vec![UPDATE]);
}

#[test]
fn warning_opens_the_report_and_an_update_that_stops_being_ready_closes_the_offer() {
    let mut caption = CaptionUpdates::default();
    caption.observe(feed(Some(&failed())));
    assert_eq!(caption.activate(UPDATE), (false, Some(UpdateIntent::OpenReport)));
    assert_eq!(caption.activate(OFFER), (false, None));

    caption.observe(feed(Some(&ready())));
    caption.activate(UPDATE);
    assert!(caption.observe(feed(None)));
    assert!(shown(&caption.view(messages("en"))).is_empty());
    caption.observe(feed(Some(&ready())));
    assert_eq!(shown(&caption.view(messages("en"))), vec![UPDATE]);
}

#[test]
fn saving_replaces_everything_with_one_line_until_the_close_settles() {
    let en = messages("en");
    let mut caption = CaptionUpdates::default();
    caption.observe(feed(Some(&ready())));
    caption.activate(UPDATE);
    caption.notify(Notice::NotSaved { workspace: true, inputs: false });
    assert!(caption.observe(UpdateFeed::read(
        Some(&ready()),
        Some(saving()),
        WorkflowActivity::Idle
    )));
    let view = caption.view(en);
    assert_eq!(shown(&view), vec![LINE]);
    assert_eq!(view.line.text, "Saving your workspace…");
    assert_eq!(caption.activate(UPDATE), (false, None));
    assert_eq!(caption.activate(DISMISS), (false, None));

    let outcome = CloseSaveOutcome {
        reason: CloseReason::Close,
        workspace: SaveVerdict::Saved,
        inputs: SaveVerdict::Saved,
    };
    let recording = Some(CloseSaveProgress::Recording(outcome));
    assert!(UpdateFeed::read(None, recording, WorkflowActivity::Idle).saving);
    let settled = Some(CloseSaveProgress::Settled(outcome));
    assert!(!UpdateFeed::read(None, settled, WorkflowActivity::Idle).saving);
}

#[test]
fn the_not_saved_notice_is_one_line_the_user_can_dismiss() {
    let en = messages("en");
    let notice = NotSavedLastTime {
        reason: CloseReason::Close,
        items: vec![
            UnsavedItem::Workspace(UnsavedCause::Interrupted { unsaved_for: Duration::ZERO }),
            UnsavedItem::InputCanvas(UnsavedCause::Interrupted { unsaved_for: Duration::ZERO }),
        ],
    };
    let mut caption = CaptionUpdates::default();
    assert!(caption.notify(Notice::not_saved(&notice)));
    assert!(!caption.notify(Notice::not_saved(&notice)));
    let view = caption.view(en);
    assert_eq!(shown(&view), vec![LINE, DISMISS]);
    assert_eq!(view.line.text, en.closing.not_saved_both);
    assert_eq!(view.dismiss.text, "Dismiss");
    assert_eq!(caption.activate(DISMISS), (true, None));
    assert!(shown(&caption.view(en)).is_empty());

    let inputs_only = NotSavedLastTime {
        reason: CloseReason::Close,
        items: vec![UnsavedItem::InputCanvas(UnsavedCause::Interrupted {
            unsaved_for: Duration::ZERO,
        })],
    };
    caption.notify(Notice::not_saved(&inputs_only));
    assert_eq!(caption.view(messages("ja")).line.text, messages("ja").closing.not_saved_inputs);
}
