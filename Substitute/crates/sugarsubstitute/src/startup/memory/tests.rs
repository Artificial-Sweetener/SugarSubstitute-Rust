// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

use super::*;
use std::{
    io,
    sync::{Arc, Mutex},
    time::Duration,
};
use sugarsubstitute_execution::memory_budget::{PressureLevel, SystemMemorySample};

const GIB: u64 = 1024 * 1024 * 1024;

/// Scripted samples that record which processes each sample measured.
#[derive(Clone, Default)]
struct Scripted(Arc<Mutex<(SystemMemorySample, Vec<Vec<u32>>)>>);

impl Scripted {
    fn set_available(&self, bytes: u64) {
        if let Ok(mut state) = self.0.lock() {
            state.0.available_bytes = bytes;
        }
    }
}

impl SystemMemory for Scripted {
    fn sample(&self, processes: &[u32]) -> io::Result<SystemMemorySample> {
        let mut state = self.0.lock().map_err(|_| io::Error::other("poisoned"))?;
        state.1.push(processes.to_vec());
        Ok(state.0)
    }
}

fn scripted() -> Scripted {
    let source = Scripted::default();
    if let Ok(mut state) = source.0.lock() {
        state.0 = SystemMemorySample {
            total_bytes: 64 * GIB,
            available_bytes: 40 * GIB,
            commit_available_bytes: Some(64 * GIB),
            footprint_bytes: GIB,
            ..SystemMemorySample::default()
        };
    }
    source
}

#[test]
fn the_governor_samples_when_due_and_reports_only_changed_decisions() -> Result<(), Box<dyn Error>>
{
    let source = scripted();
    let mut governor = MemoryGovernor::new(Box::new(source.clone()), MemoryPreference::default());
    assert_eq!(governor.total_bytes()?, 64 * GIB);
    let start = governor.next_deadline();
    let first = governor.poll(start, Moment::from_nanos(0), 0, false)?;
    assert_eq!(first.map(|decision| decision.level), Some(PressureLevel::Normal));
    assert_eq!(governor.next_deadline(), start + Duration::from_secs(2));
    // Not yet due: no sample is taken.
    assert!(
        governor.poll(start + Duration::from_secs(1), Moment::from_nanos(1), 0, false)?.is_none()
    );
    // Due again with nothing changed: sampled, but no new decision.
    let unchanged =
        governor.poll(start + Duration::from_secs(2), Moment::from_nanos(2), 0, false)?;
    assert!(unchanged.is_none());
    source.set_available(GIB);
    let pressed = governor.poll(start + Duration::from_secs(4), Moment::from_nanos(4), 0, false)?;
    assert_eq!(
        pressed.map(|decision| (decision.level, decision.cache_bytes)),
        Some((PressureLevel::Critical, 0))
    );
    // Pressure samples more often.
    assert_eq!(
        governor.next_deadline(),
        start + Duration::from_secs(4) + Duration::from_millis(500)
    );
    Ok(())
}

#[test]
fn accepted_reload_fences_pending_and_future_browser_work_before_native_retirement()
-> Result<(), Box<dyn Error>> {
    use crate::startup::{PreparedStartup, platform_tests};
    use sugarsubstitute_application::startup::StartupSession;
    use sugarsubstitute_contracts::{
        configuration::*,
        execution::{Generation, MemoryLease, MemoryRelease},
        memory::MemoryMessage,
        panel::PanelSession,
        startup::{LaunchId, ReadinessReceipt, StartupEvent, StartupObservation},
    };
    use sugarsubstitute_desktop::{
        DesktopStartupObservation, MainReloadReason, PanelIngressReceipt, StartupWindowContent,
    };

    struct Release;
    impl MemoryRelease for Release {
        fn release(&self, _: u64) {}
    }

    let mut host = platform_tests::host()?;
    let launch = host.launch_id();
    let origin = LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?;
    let mut session = StartupSession::new(
        launch,
        Generation::new(1)?,
        StartupConfiguration::new(
            ConnectionConfiguration::Attach(origin),
            ProfileId::new("reload-memory".into())?,
            Presentation::new("en", Theme::Dark, true)?,
        ),
    );
    for observation in [
        StartupObservation::SplashPresented,
        StartupObservation::ConnectionReady(origin),
        StartupObservation::FrontendNavigationStarted(Generation::new(1)?),
    ] {
        session.observe(StartupEvent::new(launch, observation))?;
    }
    let receipt = ReadinessReceipt::parse(br#"{"schema_version":1,"launch_id":1,"navigation_generation":1,"outcome":{"status":"ready","evidence":{"frontend_version":"1.0.0","sugarcubes_version":"1.0.0","app_graph_ready":true,"canvas_connected":true,"loading_finished":true,"sugarcubes_registered":true,"sugarcubes_ready":true}}}"#)?;
    session.observe(StartupEvent::new(launch, StartupObservation::FrontendReceipt(receipt)))?;
    session.observe(StartupEvent::new(launch, StartupObservation::SplashRetired))?;
    session.observe(StartupEvent::new(launch, StartupObservation::MainRevealed))?;
    host.startup = Some(platform_tests::Prepared { session, released: false });
    let identity = host.main_identity().ok_or("Current main absent")?;
    assert_eq!(
        host.startup.as_ref().map(|startup| startup.session().phase()),
        Some(sugarsubstitute_application::startup::StartupPhase::Running)
    );
    let lease = MemoryLease::new(1024 * 1024, Box::new(Release));
    let mut governor = MemoryGovernor::new(Box::new(scripted()), MemoryPreference::default());
    governor.page.observe(
        identity,
        MemoryMessage::Usage {
            session: PanelSession::for_main(identity)?,
            used_bytes: 1,
            demand_bytes: 2,
            consumers: Vec::new(),
        },
        lease.clone(),
        lease.claim(PanelIngressReceipt::STORAGE_BYTES)?,
    )?;
    let sample = Instant::now().max(governor.next_deadline());
    governor.poll(sample, Moment::from_nanos(0), 0, false)?;
    host.memory = Some(governor);

    for foreign in [
        sugarsubstitute_contracts::startup::MainIdentity { launch: LaunchId::new(2)?, ..identity },
        sugarsubstitute_contracts::startup::MainIdentity {
            navigation: Generation::new(2)?,
            ..identity
        },
    ] {
        assert!(
            host.observe(DesktopStartupObservation::MainReloadRequested {
                identity: foreign,
                reason: MainReloadReason::Navigation,
            })
            .is_err()
        );
        assert_ne!(
            host.memory.as_ref().ok_or("Governor absent")?.page.usage(),
            page::PageUsage::default()
        );
    }
    let mut measured = false;
    while let Some(command) = host.take_command()? {
        measured |= matches!(command, DesktopStartupCommand::MeasureBrowser(current) if current == identity);
    }
    assert!(measured);
    host.memory.as_mut().ok_or("Governor absent")?.poll(
        sample + Duration::from_secs(30),
        Moment::from_nanos(30_000_000_000),
        0,
        false,
    )?;
    host.observe(DesktopStartupObservation::MainReloadRequested {
        identity,
        reason: MainReloadReason::Navigation,
    })?;
    assert!(host.take_command()?.is_none());
    let governor = host.memory.as_mut().ok_or("Governor absent")?;
    governor.poll(
        sample + Duration::from_secs(60),
        Moment::from_nanos(60_000_000_000),
        0,
        false,
    )?;
    assert!(host.take_command()?.is_none());
    assert_eq!(
        host.memory.as_ref().ok_or("Governor absent")?.page.usage(),
        page::PageUsage::default()
    );
    Ok(())
}
