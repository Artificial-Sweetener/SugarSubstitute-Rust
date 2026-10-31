// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve native reply ordering and bounded admission across partial delivery.

use super::*;
use sugarsubstitute_contracts::startup::LaunchId;

/// Distinguish queued commands without constructing an unrelated canvas or native window.
fn command(sequence: u64) -> Result<DesktopStartupCommand, Box<dyn Error>> {
    Ok(DesktopStartupCommand::RetireSplash(LaunchId::new(sequence)?))
}

/// Observe the transferred command identity at the same boundary used by the UI pump.
fn delivered(surface: &mut NativeSurface) -> Result<u64, Box<dyn Error>> {
    let Some(DesktopStartupCommand::RetireSplash(launch)) = surface.take_command() else {
        return Err("Queued command was lost or replaced".into());
    };
    Ok(launch.get())
}

#[test]
fn surface_replies_keep_admission_order_when_new_replies_arrive_between_drains()
-> Result<(), Box<dyn Error>> {
    let mut surface = NativeSurface::new();
    surface.enqueue(command(1)?)?;
    surface.enqueue(command(2)?)?;
    assert_eq!(delivered(&mut surface)?, 1);
    surface.enqueue(command(3)?)?;
    assert_eq!(delivered(&mut surface)?, 2);
    assert_eq!(delivered(&mut surface)?, 3);
    assert!(surface.take_command().is_none());
    surface.enqueue(command(4)?)?;
    assert_eq!(delivered(&mut surface)?, 4);
    Ok(())
}

#[test]
fn surface_replies_reject_overload_without_displacing_older_commands() -> Result<(), Box<dyn Error>>
{
    let mut surface = NativeSurface::new();
    for sequence in 1..=8 {
        surface.enqueue(command(sequence)?)?;
    }
    assert!(surface.enqueue(command(9)?).is_err());
    assert_eq!(delivered(&mut surface)?, 1);
    surface.enqueue(command(10)?)?;
    for sequence in 2..=8 {
        assert_eq!(delivered(&mut surface)?, sequence);
    }
    assert_eq!(delivered(&mut surface)?, 10);
    assert!(surface.take_command().is_none());
    surface.enqueue(command(11)?)?;
    surface.close(None)?;
    assert!(surface.take_command().is_none());
    Ok(())
}
