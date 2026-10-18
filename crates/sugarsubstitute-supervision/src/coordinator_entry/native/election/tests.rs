// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual election decision with real job contexts and fake filesystem outcomes.
use super::*;
use crate::coordinator_entry::fixture::Harness;
use sugarsubstitute_contracts::{
    execution::{ExecutionPort, JobOutcome},
    lifecycle::{CoordinatorKey, NamespaceId},
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
struct Files {
    entry: Option<[u8; 24]>,
    coordinator: Option<[u8; 24]>,
    original: [u8; 128],
    current: [u8; 128],
    calls: Vec<&'static str>,
    entry_held: bool,
    coordinator_held: bool,
    denied: bool,
}
impl Files {
    fn new() -> Result<Self, ClaimError> {
        let original = Record::coordinator(CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])
                .map_err(|cause| ClaimError::new("test_epoch", cause))?,
        })
        .encode();
        Ok(Self {
            entry: Some([9; 24]),
            coordinator: Some([11; 24]),
            original,
            current: original,
            calls: Vec::new(),
            entry_held: false,
            coordinator_held: false,
            denied: false,
        })
    }
}
impl ElectionBoundary for Files {
    fn acquire_entry(&mut self) -> Result<Option<[u8; 24]>, ClaimError> {
        self.calls.push("entry");
        self.entry_held = self.entry.is_some();
        Ok(self.entry)
    }
    fn read_entry(&mut self) -> Result<RecordRead<128>, ClaimError> {
        self.calls.push("read");
        Ok(RecordRead::Present(self.original))
    }
    fn acquire_coordinator(&mut self) -> Result<Option<[u8; 24]>, ClaimError> {
        self.calls.push("coordinator");
        assert!(self.entry_held);
        if self.denied {
            return Err(ClaimError::new(
                "coordinator_security",
                std::io::Error::from(std::io::ErrorKind::PermissionDenied),
            ));
        }
        self.coordinator_held = self.coordinator.is_some();
        Ok(self.coordinator)
    }
    fn read_coordinator(&mut self) -> Result<RecordRead<128>, ClaimError> {
        self.calls.push("reread");
        assert!(self.entry_held && self.coordinator_held);
        Ok(RecordRead::Present(self.current))
    }
}
fn run(body: impl FnOnce(&JobContext) -> Result<(), ClaimError> + Send + 'static) -> TestResult {
    let mut actual = Harness::new()?;
    let spec = actual.setup_spec()?;
    let mut job =
        actual.client.submit_blocking(&actual.scope, spec, move |context| body(&context))?;
    let completion = actual.consume_job(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Success(())), "{completion:?}");
    Ok(())
}
#[test]
fn held_recovery_requires_both_retained_exclusions_and_exact_second_read() -> TestResult {
    run(|context| {
        let mut files = Files::new()?;
        let result = decide(&mut files, NamespaceId::new([1; 32]), context)?;
        assert!(
            matches!(result,Elected::RecoveredHeld {record,inode,coordinator_inode} if record==files.original&&inode==[9;24]&&coordinator_inode==[11;24])
        );
        assert!(files.entry_held && files.coordinator_held);
        assert_eq!(files.calls, ["entry", "read", "coordinator", "reread"]);
        Ok(())
    })
}
#[test]
fn busy_entry_or_coordinator_keeps_live_candidate_without_recovery() -> TestResult {
    run(|context| {
        for entry_busy in [true, false] {
            let mut files = Files::new()?;
            if entry_busy {
                files.entry = None;
            } else {
                files.coordinator = None;
            }
            assert!(
                matches!(decide(&mut files,NamespaceId::new([1;32]),context)?,Elected::Candidate(epoch) if epoch.bytes()==&[2;16])
            );
            assert_eq!(files.current, files.original);
            assert!(!files.coordinator_held);
            assert_eq!(files.calls.len(), if entry_busy { 2 } else { 3 });
        }
        Ok(())
    })
}
#[test]
fn malformed_or_foreign_held_bytes_never_attempt_coordinator_recovery() -> TestResult {
    run(|context| {
        for malformed in [true, false] {
            let mut files = Files::new()?;
            if malformed {
                files.original[0] = 0;
            } else {
                files.original = Record::coordinator(CoordinatorKey {
                    namespace: NamespaceId::new([7; 32]),
                    epoch: CoordinatorEpoch::new([2; 16])
                        .map_err(|cause| ClaimError::new("test_epoch", cause))?,
                })
                .encode();
            }
            assert!(matches!(
                decide(&mut files, NamespaceId::new([1; 32]), context)?,
                Elected::Degraded(_)
            ));
            assert_eq!(files.calls, ["entry", "read"]);
            assert!(!files.coordinator_held);
        }
        Ok(())
    })
}
#[test]
fn changed_predecessor_preserves_both_adopted_exclusions_without_grant() -> TestResult {
    run(|context| {
        let mut files = Files::new()?;
        files.current[44] = 3;
        assert!(decide(&mut files, NamespaceId::new([1; 32]), context).is_err());
        assert!(files.entry_held && files.coordinator_held);
        assert_eq!(files.calls, ["entry", "read", "coordinator", "reread"]);
        Ok(())
    })
}
#[test]
fn genuine_coordinator_security_failure_preserves_cause_and_stops_readback() -> TestResult {
    run(|context| {
        let mut files = Files::new()?;
        files.denied = true;
        let cause = match decide(&mut files, NamespaceId::new([1; 32]), context) {
            Err(cause) => cause,
            Ok(_) => return Err(ClaimError::invalid("security_accepted")),
        };
        assert_eq!(
            cause
                .source()
                .and_then(|cause| cause.downcast_ref::<std::io::Error>())
                .map(std::io::Error::kind),
            Some(std::io::ErrorKind::PermissionDenied)
        );
        assert!(files.entry_held);
        assert!(!files.coordinator_held);
        assert_eq!(files.calls.len(), 3);
        Ok(())
    })
}

#[test]
fn retiring_coordinator_requires_a_new_exclusion_observation_before_released_election() -> TestResult
{
    run(|context| {
        let mut files = Files::new()?;
        files.entry = None;
        assert!(matches!(
            decide(&mut files, NamespaceId::new([1; 32]), context)?,
            Elected::Candidate(_)
        ));
        assert!(!files.entry_held && !files.coordinator_held);
        files.entry = Some([9; 24]);
        files.coordinator = None;
        assert!(matches!(
            decide(&mut files, NamespaceId::new([1; 32]), context)?,
            Elected::Candidate(_)
        ));
        assert!(files.entry_held && !files.coordinator_held);
        let mut released = Record::decode(&files.original)?;
        released.state = RecordState::Released;
        files.original = released.encode();
        files.current = files.original;
        files.calls.clear();
        assert!(matches!(decide(&mut files, NamespaceId::new([1; 32]), context)?,
            Elected::OrdinaryReleased { record, inode } if record == files.original && inode == [9;24]));
        assert_eq!(files.calls, ["entry", "read"]);
        Ok(())
    })
}
