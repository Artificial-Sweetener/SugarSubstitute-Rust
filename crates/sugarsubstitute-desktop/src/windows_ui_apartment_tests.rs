// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Injected apartment boundaries exercise the actual ownership decisions without native calls.

use super::*;
use std::{cell::Cell, error::Error as _};
use windows::Win32::Foundation::{E_ACCESSDENIED, E_FAIL};

struct Fake {
    query: Result<APTTYPE, Error>,
    acquisition: Result<(), Error>,
    fail_release: bool,
    acquired: Rc<Cell<u32>>,
    released: Rc<Cell<u32>>,
}
impl Boundary for Fake {
    fn query(&mut self) -> Result<APTTYPE, Error> {
        self.query.clone()
    }
    fn acquire(&mut self) -> Result<(), Error> {
        self.acquired.set(self.acquired.get() + 1);
        self.acquisition.clone()
    }
    fn release(&mut self) -> Result<(), Error> {
        if self.fail_release {
            return Err(Error::from(RPC_E_WRONG_THREAD));
        }
        self.released.set(self.released.get() + 1);
        Ok(())
    }
}
fn fake(query: Result<APTTYPE, Error>) -> Fake {
    Fake {
        query,
        acquisition: Ok(()),
        fail_release: false,
        acquired: Rc::new(Cell::new(0)),
        released: Rc::new(Cell::new(0)),
    }
}

#[test]
fn proven_existing_sta_is_borrowed_and_never_balanced_by_this_owner() -> Result<(), DesktopError> {
    for kind in [APTTYPE_STA, APTTYPE_MAINSTA] {
        let boundary = fake(Ok(kind));
        let acquired = boundary.acquired.clone();
        let released = boundary.released.clone();
        let mut owner = Owner::enter(boundary)?;
        owner.release()?;
        owner.release()?;
        drop(owner);
        assert_eq!(acquired.get(), 0);
        assert_eq!(released.get(), 0);
    }
    Ok(())
}

#[test]
fn acquired_reference_is_released_once_only_by_explicit_teardown() -> Result<(), DesktopError> {
    let boundary = fake(Err(Error::from(CO_E_NOTINITIALIZED)));
    let acquired = boundary.acquired.clone();
    let released = boundary.released.clone();
    let mut owner = Owner::enter(boundary)?;
    assert_eq!(acquired.get(), 1);
    assert_eq!(released.get(), 0);
    owner.release()?;
    owner.release()?;
    drop(owner);
    assert_eq!(released.get(), 1);
    Ok(())
}

#[test]
fn failed_query_or_acquisition_preserves_original_hresult() {
    for acquire in [false, true] {
        let mut boundary =
            fake(Err(Error::from(if acquire { CO_E_NOTINITIALIZED } else { E_ACCESSDENIED })));
        boundary.acquisition = Err(Error::from(E_FAIL));
        let acquired = boundary.acquired.clone();
        let result = Owner::enter(boundary);
        assert!(result.is_err());
        if let Err(cause) = result {
            assert_eq!(
                cause.source().and_then(|source| source.downcast_ref::<Error>()).map(Error::code),
                Some(if acquire { E_FAIL } else { E_ACCESSDENIED })
            );
        }
        assert_eq!(acquired.get(), u32::from(acquire));
    }
}

#[test]
fn mta_is_rejected_and_unresolved_drop_never_calls_native_release() -> Result<(), DesktopError> {
    let boundary = fake(Ok(windows::Win32::System::Com::APTTYPE_MTA));
    let acquired = boundary.acquired.clone();
    assert!(Owner::enter(boundary).is_err());
    assert_eq!(acquired.get(), 0);
    let boundary = fake(Err(Error::from(CO_E_NOTINITIALIZED)));
    let released = boundary.released.clone();
    drop(Owner::enter(boundary)?);
    assert_eq!(released.get(), 0);
    Ok(())
}

#[test]
fn failed_release_keeps_ownership_until_checked_retry() -> Result<(), DesktopError> {
    let mut boundary = fake(Err(Error::from(CO_E_NOTINITIALIZED)));
    boundary.fail_release = true;
    let released = boundary.released.clone();
    let mut owner = Owner::enter(boundary)?;
    assert!(owner.release().is_err());
    assert!(owner.ownership == Ownership::Acquired);
    assert_eq!(released.get(), 0);
    owner.boundary.fail_release = false;
    owner.release()?;
    assert_eq!(released.get(), 1);
    Ok(())
}
