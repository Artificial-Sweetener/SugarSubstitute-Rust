// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound raw CDP completion text before allocation and retain real callback obligations.

use crate::{
    DesktopError,
    main_events::{Command, Events, copy_utf16},
    main_native_delivery::OwnedDelivery,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::{HRESULT, HSTRING, PCWSTR, implement};

pub(super) struct Operations {
    pending: RefCell<[Option<Operation>; 4]>,
    next: Cell<u64>,
    events: Rc<Events>,
}

/// Completed owned deliveries remain occupied until actor validation outside native callbacks.
struct Operation {
    id: u64,
    completed: bool,
    delivery: Option<OwnedDelivery>,
}
impl Operations {
    pub(super) fn new(events: Rc<Events>) -> Result<Rc<Self>, DesktopError> {
        Ok(Rc::new(Self {
            pending: RefCell::new(std::array::from_fn(|_| None)),
            next: Cell::new(1),
            events,
        }))
    }
    fn arm(&self, delivery: Option<OwnedDelivery>) -> Result<u64, DesktopError> {
        let mut pending = self.pending.borrow_mut();
        let slot = pending.iter_mut().find(|slot| slot.is_none()).ok_or_else(|| {
            DesktopError::message("submit_main_command", "command capacity exhausted")
        })?;
        let id = self.next.get();
        self.next.set(id.checked_add(1).ok_or_else(|| {
            DesktopError::message("submit_main_command", "command identity exhausted")
        })?);
        *slot = Some(Operation { id, completed: false, delivery });
        Ok(id)
    }
    fn completed(&self, id: u64) {
        let mut pending = self.pending.borrow_mut();
        if let Some(slot) = pending
            .iter_mut()
            .find(|slot| slot.as_ref().is_some_and(|value| value.id == id && !value.completed))
        {
            if let Some(value) = slot {
                if value.delivery.is_some() {
                    value.completed = true;
                } else {
                    *slot = None;
                }
            }
        } else {
            drop(pending);
            self.events.fail(DesktopError::message(
                "complete_main_command",
                "duplicate or foreign native completion",
            ));
        }
    }
    pub(super) fn pending(&self) -> usize {
        self.pending.borrow().iter().filter(|slot| slot.is_some()).count()
    }

    /// Transfer one actual completed delivery to its actor for result and navigation validation.
    pub(super) fn take_delivery(&self, id: u64) -> Result<OwnedDelivery, DesktopError> {
        let mut pending = self.pending.borrow_mut();
        let slot = pending
            .iter_mut()
            .find(|slot| slot.as_ref().is_some_and(|value| value.id == id && value.completed))
            .ok_or_else(|| {
                DesktopError::message("complete_panel_delivery", "delivery is not completed")
            })?;
        slot.take().and_then(|value| value.delivery).ok_or_else(|| {
            DesktopError::message("complete_panel_delivery", "owned delivery is absent")
        })
    }

    /// Release only callbacks that actually returned while admission is being retired.
    pub(super) fn retire_completed(&self) {
        for slot in self.pending.borrow_mut().iter_mut() {
            if slot.as_ref().is_some_and(|value| value.completed) {
                *slot = None;
            }
        }
    }

    /// Reserve two command slots for current-document panel and lifecycle traffic.
    pub(super) fn admits_surface_delivery(&self) -> bool {
        self.pending() < 2
    }
}

#[implement(ICoreWebView2CallDevToolsProtocolMethodCompletedHandler)]
struct Completed {
    owner: Rc<Operations>,
    id: u64,
    kind: Command,
}
impl ICoreWebView2CallDevToolsProtocolMethodCompletedHandler_Impl for Completed_Impl {
    fn Invoke(&self, errorcode: HRESULT, result: &PCWSTR) -> windows::core::Result<()> {
        let _callback = self.owner.events.callback();
        if let Err(cause) = errorcode.ok() {
            self.owner.events.fail(DesktopError::new("main_cdp_completion", cause));
        } else if result.is_null() {
            self.owner.events.fail(DesktopError::new(
                "main_cdp_completion",
                windows::core::Error::from(windows::Win32::Foundation::E_POINTER),
            ));
        } else {
            self.owner.events.record_command(self.kind, |target| {
                copy_utf16(target, |index| {
                    // SAFETY: WebView2 retains a valid borrowed NUL string throughout Invoke.
                    // The scan is bounded before any authored copy; this callback never frees it.
                    unsafe { result.as_ptr().add(index).read() }
                })
            });
        }
        self.owner.completed(self.id);
        Ok(())
    }
}

pub(super) fn call(
    core: &ICoreWebView2,
    owner: &Rc<Operations>,
    kind: Command,
    method: &str,
    parameters: &str,
) -> Result<(), DesktopError> {
    if method.len() > 128
        || parameters.len() > 8192
        || method.contains('\0')
        || parameters.contains('\0')
    {
        return Err(DesktopError::message("submit_main_command", "command input bound exceeded"));
    }
    // HSTRING copies are bounded and covered by the caller's retained receiver metadata claim.
    let method = HSTRING::from(method);
    let parameters = HSTRING::from(parameters);
    let id = owner.arm(None)?;
    let handler: ICoreWebView2CallDevToolsProtocolMethodCompletedHandler =
        Completed { owner: owner.clone(), id, kind }.into();
    // SAFETY: The current UI thread owns core/handler. Arm before a possibly synchronous
    // callback. A failed submission cannot fabricate callback quiescence: its slot stays owned.
    unsafe { core.CallDevToolsProtocolMethod(&method, &parameters, &handler) }
        .map_err(|cause| DesktopError::new("submit_main_command", cause))
}

/// Move original response custody into the armed slot before a possibly synchronous native call.
pub(super) fn call_owned(
    core: &ICoreWebView2,
    owner: &Rc<Operations>,
    parameters: &str,
    delivery: OwnedDelivery,
) -> Result<(), DesktopError> {
    if parameters.len() > 8192 || parameters.contains('\0') {
        return Err(DesktopError::message("submit_main_command", "command input bound exceeded"));
    }
    let context = delivery.context;
    let id = owner.arm(Some(delivery))?;
    let handler: ICoreWebView2CallDevToolsProtocolMethodCompletedHandler =
        Completed { owner: owner.clone(), id, kind: Command::OwnedPanelDelivery { context, id } }
            .into();
    let method = HSTRING::from("Runtime.evaluate");
    let parameters = HSTRING::from(parameters);
    // SAFETY: This UI apartment retains core and the prearmed handler. Failed submission
    // leaves its complete response/claim in the slot until an actual callback or retirement.
    unsafe { core.CallDevToolsProtocolMethod(&method, &parameters, &handler) }
        .map_err(|cause| DesktopError::new("submit_main_command", cause))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_contracts::execution::{MemoryLease, MemoryRelease};

    struct Release;
    impl MemoryRelease for Release {
        fn release(&self, _: u64) {}
    }

    /// Construct retained control custody without constructing a native window or scheduler.
    fn delivery(lease: &MemoryLease) -> Result<OwnedDelivery, Box<dyn std::error::Error>> {
        use sugarsubstitute_contracts::{
            execution::Generation,
            lifecycle::*,
            startup::{LaunchId, MainIdentity},
        };
        let identity = MainIdentity {
            run: RunKey {
                owner: CoordinatorKey {
                    namespace: NamespaceId::new([7; 32]),
                    epoch: CoordinatorEpoch::new([8; 16])?,
                },
                instance: InstanceId::new(1)?,
                generation: RunGeneration::new(1)?,
            },
            launch: LaunchId::new(1)?,
            navigation: Generation::new(1)?,
        };
        Ok(OwnedDelivery {
            context: 17,
            ticket: crate::main_panel_ingress::Ticket(1),
            delivery: crate::main_native_delivery::NativeDelivery::Ingress(
                crate::PanelIngressReceipt::control(identity, lease.claim(64 * 1024)?)?,
            ),
        })
    }

    #[test]
    fn actual_callback_keeps_original_credit_until_actor_takes_the_completed_delivery()
    -> Result<(), Box<dyn std::error::Error>> {
        let event_lease = MemoryLease::new(4 * 1024 * 1024, Box::new(Release));
        let events = Events::new(&event_lease, || {})?;
        let operations = Operations::new(events.clone())?;
        let payload_lease = MemoryLease::new(64 * 1024, Box::new(Release));
        let id = operations.arm(Some(delivery(&payload_lease)?))?;
        assert!(payload_lease.claim(1).is_err());
        assert!(operations.take_delivery(id).is_err());
        let handler: ICoreWebView2CallDevToolsProtocolMethodCompletedHandler = Completed {
            owner: operations.clone(),
            id,
            kind: Command::OwnedPanelDelivery { context: 17, id },
        }
        .into();
        let response: Vec<_> =
            r#"{"result":{"type":"boolean","value":true}}"#.encode_utf16().chain([0]).collect();
        // SAFETY: The test retains the real callback and its valid borrowed NUL-terminated response.
        unsafe { handler.Invoke(HRESULT(0), PCWSTR(response.as_ptr())) }?;
        assert!(!events.in_callback());
        assert_eq!(operations.pending(), 1);
        assert!(payload_lease.claim(1).is_err());
        let completed = operations.take_delivery(id)?;
        assert_eq!(completed.context, 17);
        assert_eq!(operations.pending(), 0);
        assert!(payload_lease.claim(1).is_err());
        let claim = completed.delivery.into_claim();
        assert!(payload_lease.claim(1).is_err());
        drop(claim);
        assert!(payload_lease.claim(64 * 1024).is_ok());
        Ok(())
    }

    #[test]
    fn pending_submission_cannot_refund_credit_or_retire_before_an_actual_callback()
    -> Result<(), Box<dyn std::error::Error>> {
        let event_lease = MemoryLease::new(4 * 1024 * 1024, Box::new(Release));
        let operations = Operations::new(Events::new(&event_lease, || {})?)?;
        let payload_lease = MemoryLease::new(64 * 1024, Box::new(Release));
        let id = operations.arm(Some(delivery(&payload_lease)?))?;
        operations.retire_completed();
        assert_eq!(operations.pending(), 1);
        assert!(operations.take_delivery(id).is_err());
        assert!(payload_lease.claim(1).is_err());
        operations.completed(id);
        assert_eq!(operations.pending(), 1);
        operations.retire_completed();
        assert_eq!(operations.pending(), 0);
        assert!(payload_lease.claim(64 * 1024).is_ok());
        Ok(())
    }

    #[test]
    fn completed_delivery_backlog_keeps_the_surface_limit_and_original_allocations()
    -> Result<(), Box<dyn std::error::Error>> {
        let event_lease = MemoryLease::new(4 * 1024 * 1024, Box::new(Release));
        let operations = Operations::new(Events::new(&event_lease, || {})?)?;
        let payload_lease = MemoryLease::new(2 * 64 * 1024, Box::new(Release));
        let first = operations.arm(Some(delivery(&payload_lease)?))?;
        let second = operations.arm(Some(delivery(&payload_lease)?))?;
        assert!(!operations.admits_surface_delivery());
        operations.completed(first);
        operations.completed(second);
        assert!(!operations.admits_surface_delivery());
        assert!(payload_lease.claim(1).is_err());
        let returned = operations.take_delivery(first)?;
        assert!(operations.admits_surface_delivery());
        assert!(payload_lease.claim(1).is_err());
        drop(returned);
        assert!(payload_lease.claim(64 * 1024).is_ok());
        operations.retire_completed();
        assert_eq!(operations.pending(), 0);
        Ok(())
    }

    #[test]
    fn duplicate_callback_cannot_replace_or_refund_completed_delivery_custody()
    -> Result<(), Box<dyn std::error::Error>> {
        let event_lease = MemoryLease::new(4 * 1024 * 1024, Box::new(Release));
        let events = Events::new(&event_lease, || {})?;
        let operations = Operations::new(events.clone())?;
        let payload_lease = MemoryLease::new(64 * 1024, Box::new(Release));
        let id = operations.arm(Some(delivery(&payload_lease)?))?;
        operations.completed(id);
        operations.completed(id);
        assert!(events.take_failure().is_some());
        assert_eq!(operations.pending(), 1);
        assert!(payload_lease.claim(1).is_err());
        operations.retire_completed();
        assert!(payload_lease.claim(64 * 1024).is_ok());
        Ok(())
    }

    #[test]
    fn surface_delivery_keeps_capacity_for_panel_commands_until_callbacks_complete()
    -> Result<(), Box<dyn std::error::Error>> {
        let lease = MemoryLease::new(4 * 1024 * 1024, Box::new(Release));
        let events = Events::new(&lease, || {})?;
        let operations = Operations::new(events)?;
        assert!(operations.admits_surface_delivery());
        let first = operations.arm(None)?;
        let second = operations.arm(None)?;
        assert!(!operations.admits_surface_delivery());
        let third = operations.arm(None)?;
        let fourth = operations.arm(None)?;
        assert!(operations.arm(None).is_err());
        operations.completed(first);
        operations.completed(second);
        assert!(!operations.admits_surface_delivery());
        operations.completed(third);
        assert!(operations.admits_surface_delivery());
        operations.completed(fourth);
        Ok(())
    }
}
