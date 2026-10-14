// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit bounded application commands exclusively while the actual native UI loop dispatches.

use super::{
    DesktopError, DesktopStartupObservation, EventLoopProxy, MainActor, NativeAction, Session,
    StartupWindowContent, activity, main, main_operation,
};
use crate::DesktopStartupCommand;

pub(super) fn drain(
    session: &mut Session,
    content: &mut impl StartupWindowContent,
    proxy: EventLoopProxy<NativeAction>,
) -> Result<(), DesktopError> {
    for _ in 0..32 {
        let Some(command) = content
            .take_command()
            .map_err(|cause| DesktopError::boxed("take_startup_command", cause))?
        else {
            return Ok(());
        };
        if session.closing
            && !matches!(
                command,
                DesktopStartupCommand::Close
                    | DesktopStartupCommand::RetireMain { .. }
                    | DesktopStartupCommand::ConfirmMainAbsent(_)
                    | DesktopStartupCommand::ProcessUiChallenge(_)
            )
        {
            return Err(DesktopError::message("startup_command", "startup admission is closed"));
        }
        match command {
            DesktopStartupCommand::ProcessUiChallenge(key) => {
                activity(session, content)?;
                if let Some(observation) =
                    session.activity.process_challenge(key, session.observers.live())?
                {
                    session.observe(content, observation)?;
                }
            }
            DesktopStartupCommand::PrepareMain(request) => {
                session.main_fence.prepare(request.identity(), content.main_identity())?;
                if !session.splash.presented
                    || session.splash.retiring
                    || request.identity().launch != content.launch_id()
                    || request.identity().run != content.run_key()
                    || session.splash.launch != request.identity().launch
                    || session.main.as_ref().is_some_and(|main| !main.retired())
                {
                    return Err(DesktopError::message(
                        "prepare_main",
                        "splash not submitted or prior main lifetime remains",
                    ));
                }
                if let Some(previous) = session.main.take() {
                    session.navigations += previous.navigations();
                    session.main_material = previous.material();
                }
                session.main_failures_reported = false;
                session.main = Some(MainActor::new(*request, proxy.clone())?);
            }
            DesktopStartupCommand::AuthorizeEnvironment(identity) => {
                #[cfg(feature = "k1-native-qualification")]
                tracing::info!(operation="main_command_dispatched", command="AuthorizeEnvironment", identity=?identity,
                    "Actual creating-thread Main command dispatch; native acceptance remains separate");
                main_operation(session, content, |owner| {
                    owner.authorize_environment(identity).map(|()| None)
                })?
            }
            DesktopStartupCommand::AuthorizeBrowser(identity) => {
                #[cfg(feature = "k1-native-qualification")]
                tracing::info!(operation="main_command_dispatched", command="AuthorizeBrowser", identity=?identity,
                    "Actual creating-thread Main command dispatch; native acceptance remains separate");
                main_operation(session, content, |owner| {
                    owner.authorize_browser(identity).map(|()| None)
                })?
            }
            DesktopStartupCommand::ProbeFrontend(identity) => {
                main_operation(session, content, |owner| owner.probe(identity).map(|()| None))?;
            }
            DesktopStartupCommand::RetireMain { identity, reason } => {
                if session.main.as_ref().is_some_and(|owner| owner.identity() == identity) {
                    let owner = main(session)?;
                    owner.observe_retirement_reason(identity, reason)?;
                    owner.close(identity)?;
                } else {
                    confirm_main_absent(session, content, identity)?;
                }
            }
            DesktopStartupCommand::ConfirmMainAbsent(identity) => {
                confirm_main_absent(session, content, identity)?
            }
            DesktopStartupCommand::RetireSplash(launch) => {
                if launch != session.splash.launch || !session.splash.presented {
                    return Err(DesktopError::message(
                        "retire_splash",
                        "splash identity stale or not submitted",
                    ));
                }
                session.splash.begin_retirement(content)?;
            }
            DesktopStartupCommand::RevealMain(identity) => {
                if !session.splash.retired || !session.splash_retirement_reported {
                    return Err(DesktopError::message(
                        "reveal_main",
                        "actual splash retirement is missing",
                    ));
                }
                #[cfg(feature = "k1-native-qualification")]
                tracing::info!(operation="main_command_dispatched", command="RevealMain", identity=?identity,
                    "Actual creating-thread Main command dispatch; native acceptance remains separate");
                main_operation(session, content, |owner| owner.reveal(identity).map(Some))?;
            }
            #[cfg(feature = "k1-native-qualification")]
            DesktopStartupCommand::CaptureMainPreview { identity, source_deadline } => {
                tracing::info!(operation="main_command_dispatched", command="CaptureMainPreview", identity=?identity,
                    "Actual creating-thread Main command dispatch; native acceptance remains separate");
                main_operation(session, content, |owner| {
                    Ok(owner.capture_preview(identity, source_deadline))
                })?;
            }
            DesktopStartupCommand::Close => session.close(content),
        }
    }
    proxy
        .send_event(NativeAction::Main)
        .map_err(|cause| DesktopError::new("rewake_startup_commands", cause))
}

/// Sample the actual creating-thread actor and application input/job custody before fencing.
fn confirm_main_absent(
    session: &mut Session,
    content: &mut impl StartupWindowContent,
    identity: crate::MainIdentity,
) -> Result<(), DesktopError> {
    let custody = match &session.main {
        None => super::super::main_fence::MainCustody::Absent,
        Some(main) if main.retired() => {
            super::super::main_fence::MainCustody::CheckedRetired(main.identity())
        }
        Some(_) => super::super::main_fence::MainCustody::Outstanding,
    };
    session.main_fence.absent(
        identity,
        content.main_identity(),
        content.main_construction_pending(),
        custody,
    )?;
    session.observe(content, DesktopStartupObservation::MainAbsent(identity))
}
