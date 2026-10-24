// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise reveal acceptance through real authorization and recovery owners with native proof ports.
use super::*;
use crate::production::parent_lifecycle::recovery::MountedDocument;
use sugarsubstitute_contracts::{
    configuration::LoopbackOrigin, lifecycle::GrantedResourceInvalidation,
};

fn authorized() -> Result<ParentLifecycle> {
    use MainAuthorizationObservation as Observation;
    let mut parent = granted()?;
    let identity = identity()?;
    let pid = std::num::NonZeroU32::new(1234).ok_or("PID absent")?;
    for (request, observation) in [
        (4, Observation::RequestedFolderAccepted { identity, next_offset_units: 12 }),
        (5, Observation::EnvironmentFolderComplete { identity }),
        (5, Observation::EnvironmentVerified { identity }),
        (5, Observation::EnvironmentAuthorizationIssued { identity }),
        (6, Observation::ControllerObserved { identity, browser_process_id: pid }),
        (6, Observation::BrowserVerified { identity, browser_process_id: pid }),
        (6, Observation::BrowserAuthorizationIssued { identity, browser_process_id: pid }),
    ] {
        parent.main.accept(binding()?, RequestId::new(request)?, observation, moment(10))?;
    }
    parent
        .recovery
        .bind_initial(
            lease()?,
            MountedDocument::new(
                LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?,
                "1.0.0",
                "1.0.0",
            )
            .map_err(|cause| cause as Box<dyn Error>)?,
        )
        .map_err(|cause| cause as Box<dyn Error>)?;
    Ok(parent)
}

#[test]
fn reveal_requires_live_native_admission_before_recording_late_startup_success() -> Result<()> {
    let mut parent = authorized()?;
    let original = parent.main.snapshot();
    assert!(
        parent
            .apply_main_reveal(
                identity()?,
                binding()?,
                RequestId::new(7)?,
                |_| Err(ParentError::ForeignPeer.into()),
                || Ok(moment(300)),
            )
            .is_err()
    );
    assert_eq!(parent.main.snapshot(), original);
    parent
        .apply_main_reveal(
            identity()?,
            binding()?,
            RequestId::new(7)?,
            |_| Ok(()),
            || Ok(moment(300)),
        )
        .map_err(|cause| cause as Box<dyn Error>)?;
    assert!(parent.main.snapshot().reveal_observed);
    assert_eq!(parent.main.snapshot().frontend_until, Some(moment(30)));
    Ok(())
}

#[test]
fn backend_outage_or_close_racing_reveal_cannot_certify_startup() -> Result<()> {
    for closing in [false, true] {
        let mut parent = authorized()?;
        if closing {
            parent.closed = true;
        } else {
            parent
                .recovery
                .invalidate(GrantedResourceInvalidation { lease: lease()?, status: 259 }, 1)
                .map_err(|cause| cause as Box<dyn Error>)?;
        }
        let queried = std::cell::Cell::new(false);
        assert!(
            !parent
                .apply_main_reveal(
                    identity()?,
                    binding()?,
                    RequestId::new(7)?,
                    |_| {
                        queried.set(true);
                        Ok(())
                    },
                    || Ok(moment(300)),
                )
                .map_err(|cause| cause as Box<dyn Error>)?
        );
        assert!(!queried.get());
        assert!(!parent.main.snapshot().reveal_observed);
        assert_eq!(parent.lease, Some(lease()?));
    }
    Ok(())
}
