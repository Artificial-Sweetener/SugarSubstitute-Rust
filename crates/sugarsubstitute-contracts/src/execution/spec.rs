// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Describe application-owned work without choosing its scheduling mechanism.

use super::{Generation, JobId, Moment, ScopeId, SurfaceId};

/// Identify the use case responsible for submitted work and diagnostics.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Operation {
    /// Prepare application startup.
    Startup,
    /// Drain an owned process pipe.
    PipeDrain,
    /// Observe readiness at a bounded external boundary.
    Readiness,
    /// Coordinate a native lifecycle operation on its owning adapter.
    NativeLifecycle,
    /// Prepare immutable presentation pixels.
    FramePrepare,
    /// Decode bounded image storage.
    ImageDecode,
    /// Perform finite infrastructure work.
    Infrastructure,
}

/// Select an independently bounded admission class.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum JobClass {
    /// Ordinary asynchronous I/O capacity.
    IoOrdinary,
    /// Reserved process, readiness and lifecycle I/O capacity.
    IoLifecycle,
    /// Ordinary finite blocking infrastructure capacity.
    InfrastructureOrdinary,
    /// Reserved finite blocking lifecycle infrastructure capacity.
    InfrastructureLifecycle,
    /// Interactive presentation preparation capacity.
    CpuInteractive,
    /// Background image decoding capacity.
    CpuDecode,
    /// Private native observations admitted independently of asynchronous I/O.
    NativeObservation,
}

/// Select the latest application-owned revision of one presentation surface.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ReplacementKey {
    /// Stable presentation surface identity.
    pub surface: SurfaceId,
    /// Monotonically increasing revision assigned by the application.
    pub revision: Generation,
}

/// Describe one application/task lifetime before creating its scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScopeSpec {
    /// Application-assigned identity advancing every accepted scope in this executor.
    pub id: ScopeId,
    /// Optional monotonic deadline in the injected clock's epoch.
    pub deadline: Option<Moment>,
}

/// Describe bounded admitted work without constructing its future or task.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JobSpec {
    /// Application-assigned identity advancing every accepted job in this executor.
    pub id: JobId,
    /// Use case responsible for this work.
    pub operation: Operation,
    /// Capacity reservation selected for this work.
    pub class: JobClass,
    /// Optional deadline, further bounded by every ancestor scope deadline.
    pub deadline: Option<Moment>,
    /// Peak retained bytes charged before invoking the task factory.
    pub reservation_bytes: u64,
    /// Required for interactive CPU work and prohibited for every other class.
    ///
    /// A newer admitted revision replaces older pending work for this surface.
    pub replacement: Option<ReplacementKey>,
}

impl JobSpec {
    /// Validate bounded replacement ownership for the selected capacity class.
    ///
    /// Interactive CPU preparation requires a surface revision. All other work
    /// classes prohibit replacement metadata and retain their own admitted outcome.
    pub const fn validates_replacement(&self) -> bool {
        match self.class {
            JobClass::CpuInteractive => self.replacement.is_some(),
            _ => self.replacement.is_none(),
        }
    }

    /// Check the authoritative operation-to-capacity-class contract.
    ///
    /// Submission additionally validates its async/blocking/CPU request kind at
    /// the execution adapter. This predicate grants no queue or memory admission.
    pub const fn validates_class(&self) -> bool {
        matches!(
            (self.operation, self.class),
            (
                Operation::Startup | Operation::PipeDrain | Operation::Readiness,
                JobClass::IoLifecycle
            ) | (
                Operation::NativeLifecycle,
                JobClass::IoLifecycle
                    | JobClass::InfrastructureLifecycle
                    | JobClass::NativeObservation
            ) | (Operation::FramePrepare, JobClass::CpuInteractive)
                | (Operation::ImageDecode, JobClass::CpuDecode)
                | (
                    Operation::Infrastructure,
                    JobClass::IoOrdinary
                        | JobClass::InfrastructureOrdinary
                        | JobClass::InfrastructureLifecycle
                )
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::IdentityError;

    #[test]
    fn use_cases_preserve_reserved_lifecycle_and_cpu_capacity() -> Result<(), IdentityError> {
        let allowed = [
            (Operation::Startup, vec![JobClass::IoLifecycle]),
            (Operation::PipeDrain, vec![JobClass::IoLifecycle]),
            (Operation::Readiness, vec![JobClass::IoLifecycle]),
            (
                Operation::NativeLifecycle,
                vec![
                    JobClass::IoLifecycle,
                    JobClass::InfrastructureLifecycle,
                    JobClass::NativeObservation,
                ],
            ),
            (Operation::FramePrepare, vec![JobClass::CpuInteractive]),
            (Operation::ImageDecode, vec![JobClass::CpuDecode]),
            (
                Operation::Infrastructure,
                vec![
                    JobClass::IoOrdinary,
                    JobClass::InfrastructureOrdinary,
                    JobClass::InfrastructureLifecycle,
                ],
            ),
        ];
        let classes = [
            JobClass::IoOrdinary,
            JobClass::IoLifecycle,
            JobClass::InfrastructureOrdinary,
            JobClass::InfrastructureLifecycle,
            JobClass::CpuInteractive,
            JobClass::CpuDecode,
            JobClass::NativeObservation,
        ];
        for (operation, expected) in allowed {
            for class in classes {
                let spec = JobSpec {
                    id: JobId::new(1)?,
                    operation,
                    class,
                    deadline: None,
                    reservation_bytes: 0,
                    replacement: None,
                };
                assert_eq!(
                    spec.validates_class(),
                    expected.contains(&class),
                    "{operation:?}/{class:?}"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn only_interactive_cpu_work_requires_and_accepts_surface_replacement()
    -> Result<(), IdentityError> {
        let replacement =
            ReplacementKey { surface: SurfaceId::new(1)?, revision: Generation::new(1)? };
        for class in [
            JobClass::IoOrdinary,
            JobClass::IoLifecycle,
            JobClass::InfrastructureOrdinary,
            JobClass::InfrastructureLifecycle,
            JobClass::CpuInteractive,
            JobClass::CpuDecode,
            JobClass::NativeObservation,
        ] {
            let mut spec = JobSpec {
                id: JobId::new(1)?,
                operation: Operation::FramePrepare,
                class,
                deadline: None,
                reservation_bytes: 0,
                replacement: None,
            };
            assert_eq!(spec.validates_replacement(), class != JobClass::CpuInteractive);
            spec.replacement = Some(replacement);
            assert_eq!(spec.validates_replacement(), class == JobClass::CpuInteractive);
        }
        Ok(())
    }
}
