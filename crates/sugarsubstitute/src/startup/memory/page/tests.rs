// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

use super::*;
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryRelease},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    panel::PanelSession,
    startup::LaunchId,
};

const GIB: u64 = 1024 * 1024 * 1024;

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

fn identity(navigation: u64) -> Result<MainIdentity, Box<dyn Error>> {
    Ok(MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(navigation)?,
    })
}

fn lease() -> MemoryLease {
    MemoryLease::new(1024 * 1024, Box::new(Release))
}

fn report(
    link: &mut PageLink,
    identity: MainIdentity,
    message: MemoryMessage,
) -> Result<Option<MemoryPreference>, Box<dyn Error>> {
    let lease = lease();
    let claim = lease.claim(PanelIngressReceipt::STORAGE_BYTES)?;
    link.observe(identity, message, lease, claim)
}

fn usage(identity: MainIdentity, used: u64, demand: u64) -> Result<MemoryMessage, Box<dyn Error>> {
    Ok(MemoryMessage::Usage {
        session: PanelSession::for_main(identity)?,
        used_bytes: used,
        demand_bytes: demand,
        consumers: Vec::new(),
    })
}

fn decision(level: PressureLevel, page_bytes: u64) -> BudgetDecision {
    BudgetDecision {
        level,
        ceiling_bytes: 16 * GIB,
        cache_bytes: 4 * page_bytes,
        native_bytes: 3 * page_bytes,
        page_bytes,
    }
}

fn drain(link: &mut PageLink) -> Vec<DesktopStartupCommand> {
    std::iter::from_fn(|| link.take_command()).collect()
}

#[test]
fn a_report_returns_its_credit_and_the_page_receives_one_allowance_per_change()
-> Result<(), Box<dyn Error>> {
    let mut link = PageLink::default();
    let page = identity(1)?;
    // No page has reported yet: nothing is owed.
    link.apply(
        decision(PressureLevel::Normal, GIB),
        MemoryPreference::default(),
        64 * GIB,
        false,
        Instant::now(),
    )?;
    assert!(link.take_command().is_none());
    assert!(report(&mut link, page, usage(page, 100, 200)?)?.is_none());
    assert_eq!(link.usage(), PageUsage { used_bytes: 100, demand_bytes: 200 });
    let now = Instant::now();
    link.apply(
        decision(PressureLevel::Normal, GIB),
        MemoryPreference::default(),
        64 * GIB,
        false,
        now,
    )?;
    let commands = drain(&mut link);
    assert!(matches!(commands[0], DesktopStartupCommand::CompletePanelIngress(_)));
    assert!(
        matches!(&commands[1], DesktopStartupCommand::SendMemory(command) if command.identity() == page)
    );
    assert!(
        matches!(commands[2], DesktopStartupCommand::MeasureBrowser(measured) if measured == page)
    );
    assert_eq!(commands.len(), 3);
    // Unchanged: nothing is resent and the next measurement waits its interval.
    link.apply(
        decision(PressureLevel::Normal, GIB),
        MemoryPreference::default(),
        64 * GIB,
        false,
        now,
    )?;
    assert!(link.take_command().is_none());
    link.apply(
        decision(PressureLevel::Elevated, GIB / 2),
        MemoryPreference::default(),
        64 * GIB,
        false,
        now,
    )?;
    assert!(matches!(drain(&mut link)[..], [DesktopStartupCommand::SendMemory(_)]));
    link.apply(
        decision(PressureLevel::Elevated, GIB / 2),
        MemoryPreference::default(),
        64 * GIB,
        false,
        now + MEASURE_INTERVAL,
    )?;
    assert!(matches!(drain(&mut link)[..], [DesktopStartupCommand::MeasureBrowser(_)]));
    Ok(())
}

#[test]
fn preferences_reach_the_budget_and_only_a_hidden_pressed_browser_trims()
-> Result<(), Box<dyn Error>> {
    let mut link = PageLink::default();
    let page = identity(1)?;
    let preference = MemoryMessage::Preference {
        session: PanelSession::for_main(page)?,
        preference: MemoryPreferenceChoice::Fixed { bytes: 8 * GIB },
    };
    assert_eq!(report(&mut link, page, preference)?, Some(MemoryPreference::Fixed(8 * GIB)));
    let now = Instant::now();
    link.apply(
        decision(PressureLevel::High, GIB),
        MemoryPreference::Fixed(8 * GIB),
        64 * GIB,
        false,
        now,
    )?;
    assert!(
        !drain(&mut link)
            .iter()
            .any(|command| matches!(command, DesktopStartupCommand::SetBrowserMemoryTarget { .. }))
    );
    link.apply(
        decision(PressureLevel::High, GIB),
        MemoryPreference::Fixed(8 * GIB),
        64 * GIB,
        true,
        now,
    )?;
    assert!(matches!(
        drain(&mut link)[..],
        [DesktopStartupCommand::SetBrowserMemoryTarget { low: true, .. }]
    ));
    link.apply(
        decision(PressureLevel::Normal, GIB),
        MemoryPreference::Fixed(8 * GIB),
        64 * GIB,
        true,
        now,
    )?;
    let commands = drain(&mut link);
    assert!(commands.iter().any(|command| matches!(
        command,
        DesktopStartupCommand::SetBrowserMemoryTarget { low: false, .. }
    )));
    Ok(())
}

#[test]
fn a_retired_or_replaced_navigation_starts_over() -> Result<(), Box<dyn Error>> {
    let mut link = PageLink::default();
    let first = identity(1)?;
    report(&mut link, first, usage(first, 1, 1)?)?;
    link.apply(
        decision(PressureLevel::Normal, GIB),
        MemoryPreference::default(),
        64 * GIB,
        false,
        Instant::now(),
    )?;
    drain(&mut link);
    // A reloaded page receives the unchanged allowance again.
    let second = identity(2)?;
    report(&mut link, second, usage(second, 1, 1)?)?;
    link.apply(
        decision(PressureLevel::Normal, GIB),
        MemoryPreference::default(),
        64 * GIB,
        false,
        Instant::now(),
    )?;
    assert!(drain(&mut link).iter().any(|command| matches!(command, DesktopStartupCommand::SendMemory(sent) if sent.identity() == second)));
    link.retire(second);
    link.apply(
        decision(PressureLevel::Elevated, GIB),
        MemoryPreference::default(),
        64 * GIB,
        false,
        Instant::now(),
    )?;
    assert!(link.take_command().is_none());
    assert_eq!(link.usage(), PageUsage::default());
    Ok(())
}
