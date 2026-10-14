// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain exact finite Core launch/timing deliveries and their original allocation credit.
use super::*;
#[path = "delivery/native_input.rs"]
mod native_input;
pub(super) use native_input::{NATIVE_INPUT_PEAK_BYTES, NativeLaunchInput};

pub(super) struct Material {
    pub(super) context: IncidentRequestContext,
    _claim: MemoryClaim,
}
pub(super) struct PreparedLaunch {
    pub(super) bytes: u64,
    _input: sugarsubstitute_execution::ChargedBlockingInput<LaunchDraft>,
    _claim: MemoryClaim,
}
impl PreparedLaunch {
    /// Borrow the actual original registered startup retained by this matched launch preparation.
    pub(super) fn startup(&self) -> &sugarsubstitute_execution::RegisteredStartupSource {
        &self._input.input().startup
    }
}
pub(super) struct LaunchDraft {
    pub(super) source: ReporterSource,
    pub(super) executable: PathBuf,
    pub(super) working: PathBuf,
    pub(super) startup: sugarsubstitute_execution::RegisteredStartupSource,
    locale: sugarsubstitute_contracts::configuration::LocaleId,
    input: Option<NativeLaunchInput>,
}
impl LaunchDraft {
    pub(super) fn new(
        source: ReporterSource,
        executable: PathBuf,
        working: PathBuf,
        startup: sugarsubstitute_execution::RegisteredStartupSource,
        locale: sugarsubstitute_contracts::configuration::LocaleId,
    ) -> Self {
        Self { source, executable, working, startup, locale, input: None }
    }
}
pub(super) struct WireCounter {
    pub(super) counter: DiagnosticDeadline,
    _claim: MemoryClaim,
}
pub(super) struct Failure {
    cause: Box<dyn Error + Send + Sync>,
    _input: Option<sugarsubstitute_execution::ChargedBlockingInput<LaunchDraft>>,
    _launch_storage: Option<MemoryClaim>,
    _original: MemoryLease,
}
#[cfg(test)]
impl Failure {
    pub(super) fn test_cause(context: &JobContext, message: &'static str) -> Self {
        Self {
            cause: Box::new(std::io::Error::other(message)),
            _original: context.memory().clone(),
            _input: None,
            _launch_storage: None,
        }
    }
}
impl std::fmt::Debug for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.cause, f)
    }
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Report receiving preparation failed")
    }
}
impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
pub(super) struct Delivery<T: Send + 'static> {
    pub(super) job: Option<OwnedJob<T, Failure>>,
    pub(super) completion: Option<JobCompletion<T, Failure>>,
    terminal: Option<TerminalKind>,
    matched: bool,
}
impl<T: Send + 'static> Delivery<T> {
    pub(super) const fn new() -> Self {
        Self { job: None, completion: None, terminal: None, matched: false }
    }
    pub(super) fn capture(&mut self, batch: &CompletionBatch) -> Result<()> {
        let Some(job) = &mut self.job else { return Ok(()) };
        for notice in &batch.notifications {
            if (notice.scope_id, notice.job_id) != (job.scope_id(), job.spec().id) {
                continue;
            }
            if self.terminal.is_some_and(|old| old != notice.kind) {
                return Err("Report preparation terminal changed".into());
            }
            self.terminal = Some(notice.kind);
        }
        if self.terminal.is_some()
            && let Some(done) = job.try_take_completion()
        {
            self.matched = done.scope_id == job.scope_id()
                && done.spec == job.spec()
                && self.terminal == Some(done.outcome.kind());
            self.completion = Some(done);
            self.job = None;
            if !self.matched {
                return Err("Report preparation completion identity changed".into());
            }
        }
        Ok(())
    }
    pub(super) fn value(&self) -> Result<Option<&T>> {
        let Some(done) = &self.completion else { return Ok(None) };
        if !self.matched {
            return Err("Unmatched report preparation completion".into());
        }
        match &done.outcome {
            JobOutcome::Success(value) => Ok(Some(value)),
            _ => Err("Actual report preparation stopped with original cause retained".into()),
        }
    }
    pub(super) fn failure(&self) -> Option<&(dyn Error + 'static)> {
        match self.completion.as_ref().map(|done| &done.outcome) {
            Some(JobOutcome::Failure(cause)) => Some(cause),
            _ => None,
        }
    }
    pub(super) fn recyclable(&self) -> Result<()> {
        if self.job.is_some() {
            return Err("Actual preparation worker remains owned".into());
        }
        if let Some(done) = &self.completion {
            if !self.matched || self.terminal != Some(done.outcome.kind()) {
                return Err("Actual preparation identity remains unmatched".into());
            }
            if !matches!(done.outcome, JobOutcome::Success(_)) {
                return Err("Original preparation failure remains retained".into());
            }
        }
        Ok(())
    }
    pub(super) fn delivered(&self) -> bool {
        self.job.is_none()
    }
    pub(super) fn cancel(&self) {
        if let Some(job) = &self.job {
            job.cancel()
        }
    }
}
pub(super) fn prepare_material(
    client: &ExecutionClient,
    original: IncidentOperationBudget,
    context: &JobContext,
) -> std::result::Result<Material, Failure> {
    let result = (|| -> std::result::Result<_, Box<dyn Error + Send + Sync>> {
        let until = original.until();
        if context.cancellation_requested() || context.deadline() != Some(until) {
            return Err("Original report preparation controls changed".into());
        }
        let now = client.now()?;
        if now >= until {
            return Err("Original report preparation deadline elapsed".into());
        }
        let _claim = context.memory().claim(16 * 1024)?;
        let elapsed = SystemTime::now().duration_since(UNIX_EPOCH)?;
        let at = IncidentUtc::from_unix_seconds(i64::try_from(elapsed.as_secs())?)?;
        Ok(Material { context: IncidentRequestContext::new(original, at), _claim })
    })();
    result.map_err(|cause| Failure {
        cause,
        _original: context.memory().clone(),
        _input: None,
        _launch_storage: None,
    })
}
pub(super) fn prepare_launch(
    mut original: sugarsubstitute_execution::ChargedBlockingInput<LaunchDraft>,
    context: JobContext,
) -> std::result::Result<PreparedLaunch, Failure> {
    let mut allocation = None;
    let result = (|| -> std::result::Result<_, Box<dyn Error + Send + Sync>> {
        if context.memory().bytes() != 512 * 1024
            || context.cancellation_requested()
            || context.deadline().is_none_or(|until| context.now() >= until)
        {
            return Err("Original Reporter launch preparation stopped".into());
        }
        allocation = Some(
            context
                .memory()
                .claim(512 * 1024 - size_of::<LaunchDraft>() as u64 - NATIVE_INPUT_PEAK_BYTES)?,
        );
        let input = original.input_mut();
        let source = input.source.alias();
        NativeLaunchInput::reserve(&context, &mut input.input)?;
        let native = input.input.as_mut().ok_or("Actual funded native launch slot missing")?;
        if native.input().is_none() {
            *native.input_mut() = Some(
                source
                    .prepare(|bootstrap| -> std::result::Result<_, Box<dyn Error + Send + Sync>> {
                        let arguments = options::arguments(bootstrap, input.locale)?;
                        let command = WindowsLaunchCommand::from_utf16(
                            &input.executable.as_os_str().encode_wide().collect::<Vec<_>>(),
                            &input.working.as_os_str().encode_wide().collect::<Vec<_>>(),
                            &arguments,
                        )?;
                        let original = input.startup.view()?;
                        let environment = original.environment();
                        let mut units = Vec::new();
                        units.try_reserve_exact(environment.unit_len())?;
                        units.extend(environment.units());
                        let env = LaunchEnvironment::from_snapshot_utf16(&units, &[])?;
                        Ok(WindowsLaunchInput::new(command, env)?)
                    })?
                    .ok_or("Actual Reporter bootstrap not prepared")??,
            );
        }
        let bytes = native
            .input()
            .as_ref()
            .ok_or("Actual retained launch input missing")?
            .required_reservation_bytes();
        source.install_input(native.input_mut())?;
        Ok(bytes)
    })();
    match (result, allocation) {
        (Ok(bytes), Some(claim)) => Ok(PreparedLaunch { bytes, _input: original, _claim: claim }),
        (result, allocation) => {
            let cause = result.err().unwrap_or_else(|| "Original launch allocation missing".into());
            Err(Failure {
                cause,
                _input: Some(original),
                _launch_storage: allocation,
                _original: context.memory().clone(),
            })
        }
    }
}
pub(super) fn prepare_counter(
    client: &ExecutionClient,
    until: Moment,
    context: &JobContext,
) -> std::result::Result<WireCounter, Failure> {
    let result = (|| -> std::result::Result<_, Box<dyn Error + Send + Sync>> {
        if context.cancellation_requested()
            || context.deadline() != Some(until)
            || client.now()? >= until
        {
            return Err("Original report counter preparation stopped".into());
        }
        let _claim = context.memory().claim(16 * 1024)?;
        let original = client.encode_counter_deadline(until)?;
        Ok(WireCounter {
            counter: DiagnosticDeadline::new(original.ticks(), original.frequency())?,
            _claim,
        })
    })();
    result.map_err(|cause| Failure {
        cause,
        _original: context.memory().clone(),
        _input: None,
        _launch_storage: None,
    })
}

const _: () = assert!(size_of::<LaunchDraft>() as u64 + NATIVE_INPUT_PEAK_BYTES < 512 * 1024);
