// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the real partial-slot transaction against deterministic native effect failures.

use super::*;
use std::{cell::RefCell, error::Error, rc::Rc};

struct Slot {
    id: usize,
    released: Rc<RefCell<Vec<usize>>>,
}
impl Drop for Slot {
    fn drop(&mut self) {
        self.released.borrow_mut().push(self.id);
    }
}
struct Port {
    calls: Vec<(char, usize)>,
    fail: Option<(char, usize)>,
    released: Rc<RefCell<Vec<usize>>>,
    callbacks_live: bool,
}
impl Port {
    fn new(fail: Option<(char, usize)>) -> Self {
        Self {
            calls: Vec::new(),
            fail,
            released: Rc::new(RefCell::new(Vec::new())),
            callbacks_live: false,
        }
    }
    fn effect(&mut self, kind: char, id: usize) -> Result<(), DesktopError> {
        self.calls.push((kind, id));
        if self.fail == Some((kind, id)) {
            return Err(DesktopError::new(
                "fake_native_effect",
                std::io::Error::other(format!("{kind}:{id}")),
            ));
        }
        Ok(())
    }
    fn slot(&self, id: usize) -> Slot {
        Slot { id, released: self.released.clone() }
    }
}
impl NativePort for Port {
    type Parent = Slot;
    type Peer = Slot;
    fn parent(&mut self) -> Result<Slot, DesktopError> {
        self.effect('P', 0)?;
        Ok(self.slot(0))
    }
    fn install_parent(&mut self, _: &Slot) -> Result<(), DesktopError> {
        self.effect('I', 0)
    }
    fn peer(&mut self, index: usize) -> Result<Slot, DesktopError> {
        self.effect('C', index + 1)?;
        Ok(self.slot(index + 1))
    }
    fn register_peer(&mut self, peer: &Slot) -> Result<(), DesktopError> {
        self.effect('R', peer.id)
    }
    fn install_peer(&mut self, peer: &Slot) -> Result<(), DesktopError> {
        self.effect('I', peer.id)
    }
    fn destroyed(&self, _: &Slot, parent: Option<&Slot>) -> bool {
        assert!(parent.is_some());
        false
    }
    fn remove_peer(&mut self, peer: &mut Slot) -> Result<(), DesktopError> {
        self.effect('M', peer.id)
    }
    fn destroy_peer(&mut self, peer: &Slot) -> Result<(), DesktopError> {
        self.effect('D', peer.id)
    }
    fn retire_parent(&mut self, _: &Slot) -> Result<(), DesktopError> {
        self.effect('D', 0)
    }
    fn callbacks_retired(&self) -> bool {
        !self.callbacks_live
    }
}

#[test]
fn inert_owner_has_no_effects_and_close_fences_all_later_creation() -> Result<(), DesktopError> {
    let mut owner = Creation::<Slot, Slot>::new(3)?;
    let mut port = Port::new(None);
    assert!(port.calls.is_empty());
    assert!(!owner.writable());
    owner.shutdown(&mut port)?;
    assert!(owner.retired(port.callbacks_retired()));
    assert!(owner.initialize(&mut port).is_err());
    assert!(port.calls.is_empty());
    Ok(())
}

#[test]
fn every_construction_failure_keeps_exact_created_slots_until_checked_cleanup()
-> Result<(), Box<dyn Error>> {
    let points = [
        ('P', 0),
        ('I', 0),
        ('C', 1),
        ('R', 1),
        ('I', 1),
        ('C', 2),
        ('R', 2),
        ('I', 2),
        ('C', 3),
        ('R', 3),
        ('I', 3),
    ];
    for point in points {
        let mut owner = Creation::new(3)?;
        let mut port = Port::new(Some(point));
        let cause = owner.initialize(&mut port).err().ok_or("Failure not reached")?;
        assert_eq!(
            cause.source().ok_or("Source lost")?.to_string(),
            format!("{}:{}", point.0, point.1)
        );
        assert!(port.released.borrow().is_empty());
        let expected_count = port
            .calls
            .iter()
            .filter(|(kind, id)| *kind == 'C' && Some((*kind, *id)) != port.fail)
            .count();
        assert_eq!(owner.peers.len(), expected_count);
        assert_eq!(owner.parent.is_some(), point != ('P', 0));
        assert!(!owner.writable());
        let attempted_calls = port.calls.len();
        assert!(owner.initialize(&mut port).is_err());
        assert_eq!(port.calls.len(), attempted_calls);
        port.fail = None;
        owner.shutdown(&mut port)?;
        assert!(owner.retired(port.callbacks_retired()));
        let mut expected: Vec<_> = (1..=expected_count).collect();
        if point != ('P', 0) {
            expected.push(0);
        }
        assert_eq!(*port.released.borrow(), expected);
        let calls = port.calls.len();
        owner.shutdown(&mut port)?;
        assert_eq!(port.calls.len(), calls);
    }
    Ok(())
}

#[test]
fn failed_peer_removal_retains_child_and_parent_then_releases_each_once() -> Result<(), DesktopError>
{
    let mut owner = Creation::new(3)?;
    let mut port = Port::new(None);
    owner.initialize(&mut port)?;
    assert!(owner.writable());
    port.fail = Some(('M', 2));
    assert!(owner.shutdown(&mut port).is_err());
    assert!(!owner.writable());
    assert!(!owner.retired(port.callbacks_retired()));
    assert_eq!(owner.peers.len(), 1);
    assert!(owner.parent.is_some());
    assert_eq!(*port.released.borrow(), [1, 3]);
    assert!(!port.calls.contains(&('D', 0)));
    assert!(owner.initialize(&mut port).is_err());
    assert!(owner.shutdown(&mut port).is_err());
    assert_eq!(*port.released.borrow(), [1, 3]);
    port.fail = None;
    owner.shutdown(&mut port)?;
    assert_eq!(*port.released.borrow(), [1, 3, 2, 0]);
    owner.shutdown(&mut port)?;
    assert_eq!(*port.released.borrow(), [1, 3, 2, 0]);
    Ok(())
}

#[test]
fn failed_parent_removal_retains_owner_after_all_children_retire() -> Result<(), DesktopError> {
    let mut owner = Creation::new(2)?;
    let mut port = Port::new(None);
    owner.initialize(&mut port)?;
    port.fail = Some(('D', 0));
    assert!(owner.shutdown(&mut port).is_err());
    assert_eq!(*port.released.borrow(), [1, 2]);
    assert!(owner.peers.is_empty());
    assert!(!owner.retired(port.callbacks_retired()));
    port.fail = None;
    owner.shutdown(&mut port)?;
    assert_eq!(*port.released.borrow(), [1, 2, 0]);
    Ok(())
}

#[test]
fn callback_reference_retention_blocks_retirement_after_empty_native_slots()
-> Result<(), DesktopError> {
    let mut owner = Creation::new(1)?;
    let mut port = Port::new(None);
    owner.initialize(&mut port)?;
    port.callbacks_live = true;
    assert!(owner.shutdown(&mut port).is_err());
    assert!(!owner.retired(port.callbacks_retired()));
    assert!(owner.parent.is_none() && owner.peers.is_empty());
    let calls = port.calls.len();
    assert!(owner.shutdown(&mut port).is_err());
    assert_eq!(port.calls.len(), calls);
    port.callbacks_live = false;
    owner.shutdown(&mut port)?;
    assert!(owner.retired(port.callbacks_retired()));
    assert_eq!(*port.released.borrow(), [1, 0]);
    Ok(())
}

#[test]
fn failed_hwnd_destroy_keeps_slot_without_repeating_successful_callback_removal()
-> Result<(), DesktopError> {
    let mut owner = Creation::new(1)?;
    let mut port = Port::new(None);
    owner.initialize(&mut port)?;
    port.fail = Some(('D', 1));
    assert!(owner.shutdown(&mut port).is_err());
    assert!(!owner.retired(port.callbacks_retired()));
    assert!(port.released.borrow().is_empty());
    assert_eq!(port.calls.iter().filter(|call| **call == ('M', 1)).count(), 1);
    assert!(owner.shutdown(&mut port).is_err());
    assert_eq!(port.calls.iter().filter(|call| **call == ('M', 1)).count(), 1);
    port.fail = None;
    owner.shutdown(&mut port)?;
    assert_eq!(*port.released.borrow(), [1, 0]);
    assert_eq!(port.calls.iter().filter(|call| **call == ('M', 1)).count(), 1);
    assert!(owner.retired(port.callbacks_retired()));
    Ok(())
}
