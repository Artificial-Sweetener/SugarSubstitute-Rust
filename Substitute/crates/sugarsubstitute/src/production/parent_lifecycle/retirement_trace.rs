// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project only the exact accepted native-retirement request at actual writer boundaries.
use super::ParentLifecycle;
use sugarsubstitute_supervision::lifecycle_protocol::WireHeader;

/// Actual composing callsites distinguish semantic staging from native issue and completion.
#[derive(Clone, Copy, Debug)]
pub(super) enum AckStage {
    Staged,
    Queued,
    Issued,
    Written,
}
impl ParentLifecycle {
    /// Trace inert matching facts; this cannot stage, issue, consume or acknowledge a request.
    pub(super) fn trace_retirement_ack(
        &self,
        header: WireHeader,
        stage: AckStage,
        native_operation: Option<u64>,
    ) {
        if self.acquisition.as_ref().and_then(|a| a.retirement_header()) != Some(header) {
            return;
        }
        tracing::info!(operation="parent_main_retirement_ack",stage=?stage,
            run=?header.run,request=?header.request,deadline_qpc=header.deadline_qpc,native_operation,
            "Original native-retirement acknowledgement boundary observed");
    }
}
#[cfg(test)]
mod tests {
    use super::super::{Result, acquisition::Acquisition, causes::BackendCauses};
    use super::*;
    use std::sync::{Arc, Mutex};
    use sugarsubstitute_contracts::{execution::Moment, lifecycle::*, startup::*};
    #[derive(Clone)]
    struct Sink(Arc<Mutex<Vec<u8>>>);
    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Sink {
        type Writer = Self;
        fn make_writer(&'a self) -> Self {
            self.clone()
        }
    }
    impl std::io::Write for Sink {
        fn write(&mut self, value: &[u8]) -> std::io::Result<usize> {
            let mut bytes =
                self.0.lock().map_err(|_| std::io::Error::other("Trace fixture poisoned"))?;
            if bytes.len().saturating_add(value.len()) > 8192 {
                return Err(std::io::Error::other("Trace fixture capacity"));
            }
            bytes.extend_from_slice(value);
            Ok(value.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    fn recorded(emit: impl FnOnce() -> Result<()>) -> Result<String> {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::fmt()
            .with_writer(Sink(bytes.clone()))
            .without_time()
            .with_ansi(false)
            .finish();
        tracing::subscriber::with_default(subscriber, emit)?;
        let bytes = bytes.lock().map_err(|_| "Trace fixture poisoned")?.clone();
        Ok(String::from_utf8(bytes)?)
    }

    fn actor() -> Result<(ParentLifecycle, WireHeader)> {
        let run = RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        };
        let binding = ClientBinding { run, connection: ConnectionId::new(1)? };
        let identity = MainIdentity {
            run,
            launch: LaunchId::new(1)?,
            navigation: sugarsubstitute_contracts::execution::Generation::new(1)?,
        };
        let original = WireHeader {
            run,
            connection: Some(binding.connection),
            request: Some(RequestId::new(1)?),
            deadline_qpc: 100,
        };
        let retirement =
            WireHeader { request: Some(RequestId::new(2)?), deadline_qpc: 200, ..original };
        let storage = crate::production::coordinator_backend::tests::storage_tests::claim(
            ParentLifecycle::storage_bytes(),
        )?;
        let mut actor = ParentLifecycle::new(
            run,
            identity.launch,
            sugarsubstitute_contracts::configuration::ProfileId::new("CloseTrace_1".into())?,
            binding.connection,
            sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeDeadline::new(
                100,
                1_000_000_000,
            )?,
            &mut Some(BackendCauses::new(storage)?),
        )?;
        let mut acquisition =
            Acquisition::new(binding, original, identity, Moment::from_nanos(100))?;
        acquisition.observe_retirement(binding, identity, retirement)?;
        actor.acquisition = Some(acquisition);
        Ok((actor, retirement))
    }
    #[test]
    fn retirement_trace_distinguishes_stage_from_native_issue_without_marking_acknowledgement()
    -> Result<()> {
        let (actor, header) = actor()?;
        let log = recorded(|| {
            for stage in [AckStage::Staged, AckStage::Queued, AckStage::Issued, AckStage::Written] {
                actor.trace_retirement_ack(header, stage, Some(9));
            }
            Ok(())
        })?;
        assert_eq!(log.matches("operation=\"parent_main_retirement_ack\"").count(), 4);
        for stage in ["Staged", "Queued", "Issued", "Written"] {
            assert_eq!(log.matches(&format!("stage={stage}")).count(), 1);
        }
        assert!(!actor.acquisition.as_ref().ok_or("Acquisition absent")?.retirement_written());
        assert_eq!(
            actor.acquisition.as_ref().ok_or("Acquisition absent")?.retirement_header(),
            Some(header)
        );
        assert!(actor.requests.next_reply().is_none());
        Ok(())
    }
    #[test]
    fn retirement_trace_rejects_foreign_full_header_without_logging_or_changing_custody()
    -> Result<()> {
        let (actor, header) = actor()?;
        let changed_run = RunKey { generation: RunGeneration::new(2)?, ..header.run };
        let log = recorded(|| {
            for changed in [
                WireHeader { run: changed_run, ..header },
                WireHeader { connection: Some(ConnectionId::new(2)?), ..header },
                WireHeader { request: Some(RequestId::new(3)?), ..header },
                WireHeader { deadline_qpc: 201, ..header },
            ] {
                actor.trace_retirement_ack(changed, AckStage::Written, Some(9));
            }
            Ok(())
        })?;
        assert!(log.is_empty());
        assert!(!actor.acquisition.as_ref().ok_or("Acquisition absent")?.retirement_written());
        assert_eq!(
            actor.acquisition.as_ref().ok_or("Acquisition absent")?.retirement_header(),
            Some(header)
        );
        Ok(())
    }
}
