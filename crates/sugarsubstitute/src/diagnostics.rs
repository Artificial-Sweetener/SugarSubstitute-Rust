// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain fallible diagnostic writes independently of the process-global subscriber.
//! Active host owners observe the same health; logging failure never recursively writes stderr.
use std::{
    error::Error,
    fmt,
    io::{self, Write},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};
mod fields;
mod queue;
#[cfg(windows)]
mod service;
#[cfg(all(windows, feature = "windows-native-host"))]
pub(crate) mod worker;

struct HealthState {
    first: OnceLock<io::Error>,
    secondary: OnceLock<io::Error>,
    omitted: AtomicU64,
}
/// Share bounded original diagnostic causes, without execution or native authority.
#[derive(Clone)]
pub(crate) struct DiagnosticHealth(Arc<HealthState>);
impl DiagnosticHealth {
    fn new() -> Self {
        Self(Arc::new(HealthState {
            first: OnceLock::new(),
            secondary: OnceLock::new(),
            omitted: AtomicU64::new(0),
        }))
    }
    /// Construct isolated data-only health for an owner test, without installing a subscriber.
    #[cfg(test)]
    pub(crate) fn fixture() -> Self {
        Self::new()
    }
    /// Inject only the external sink's data-only failure for active owner tests.
    #[cfg(all(test, windows, feature = "windows-native-host"))]
    pub(crate) fn fixture_failure(cause: io::Error) -> Self {
        let health = Self::new();
        health.retain(cause);
        health
    }
    fn retain(&self, cause: io::Error) {
        if let Err(cause) = self.0.first.set(cause)
            && self.0.secondary.set(cause).is_err()
        {
            let _previous =
                match self.0.omitted.fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                    Some(value.saturating_add(1))
                }) {
                    Ok(previous) | Err(previous) => previous,
                };
        }
    }
    /// Observe the actual retained write failure; successful logging grants no product capability.
    pub(crate) fn check(&self) -> Result<(), DiagnosticFailure> {
        if self.0.first.get().is_some() { Err(DiagnosticFailure(self.clone())) } else { Ok(()) }
    }
    /// Inspect the original OS code for bounded process-fixture evidence only.
    pub(crate) fn native_error(&self) -> Option<i32> {
        self.0.first.get().and_then(io::Error::raw_os_error)
    }
}
/// Keep original stderr I/O and its independently retained secondary cause alive through result delivery.
pub(crate) struct DiagnosticFailure(DiagnosticHealth);
impl fmt::Debug for DiagnosticFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiagnosticFailure")
            .field("native_error", &self.0.native_error())
            .field("secondary", &self.0.0.secondary.get().map(io::Error::kind))
            .field("omitted", &self.0.0.omitted.load(Ordering::Acquire))
            .finish()
    }
}
impl fmt::Display for DiagnosticFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Owned diagnostic output failed")
    }
}
impl Error for DiagnosticFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.0.0.first.get().map(|cause| cause as &dyn Error)
    }
}
struct Writer<W> {
    sink: W,
    health: DiagnosticHealth,
}
impl<W: Write> Write for Writer<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let result = self.sink.write(bytes);
        self.observe(result)
    }
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        let result = self.sink.write_all(bytes);
        self.observe(result)
    }
    fn flush(&mut self) -> io::Result<()> {
        let result = self.sink.flush();
        self.observe(result)
    }
}
impl<W> Writer<W> {
    fn observe<T>(&self, result: io::Result<T>) -> io::Result<T> {
        result.map_err(|cause| {
            let code = cause.raw_os_error();
            let kind = cause.kind();
            self.health.retain(cause);
            // The writer receives a closed rejection; the identical original error
            // and its complete source remain in the independently observed health.
            code.map_or_else(|| io::Error::from(kind), io::Error::from_raw_os_error)
        })
    }
}
struct Factory<F> {
    make: F,
    health: DiagnosticHealth,
    queue: Option<Arc<queue::Queue>>,
}
impl<'a, F, W> tracing_subscriber::fmt::MakeWriter<'a> for Factory<F>
where
    F: Fn() -> W,
    W: Write,
{
    type Writer = queue::Routed<Writer<W>>;
    fn make_writer(&'a self) -> Self::Writer {
        queue::Routed {
            sink: Writer { sink: (self.make)(), health: self.health.clone() },
            queue: self.queue.clone(),
            frame: Vec::new(),
            oversized: false,
            written: 0,
            failed: false,
        }
    }
}
fn subscriber<F, W>(
    health: DiagnosticHealth,
    make: F,
    filter: tracing_subscriber::EnvFilter,
    queue: Option<Arc<queue::Queue>>,
) -> impl tracing::Subscriber + Send + Sync
where
    F: Fn() -> W + Send + Sync + 'static,
    W: Write,
{
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .fmt_fields(fields::Fields)
        .with_ansi(false)
        .with_writer(Factory { make, health, queue })
        .log_internal_errors(false)
        .finish()
}
/// Exercise the supported formatter around a source-owned test sink without installing global state.
#[cfg(all(test, windows, feature = "windows-native-host"))]
pub(crate) fn fixture_subscriber<F, W>(make: F) -> impl tracing::Subscriber + Send + Sync
where
    F: Fn() -> W + Send + Sync + 'static,
    W: Write,
{
    subscriber(DiagnosticHealth::fixture(), make, "trace".into(), None)
}
/// Select where a process role keeps its operational history; never an account profile folder.
pub(crate) enum RecordsBase {
    /// The installation owning the running image: its root for installed images and the stable
    /// entry, or the image's own directory for an uninstalled build output.
    RunningImage,
    /// An explicit installation-local base: the installer's setup workspace.
    #[cfg(all(windows, feature = "windows-native-host"))]
    Explicit(std::path::PathBuf),
}
impl RecordsBase {
    fn resolve(self) -> std::io::Result<std::path::PathBuf> {
        match self {
            #[cfg(all(windows, feature = "windows-native-host"))]
            Self::Explicit(base) => Ok(base),
            Self::RunningImage => {
                let image = std::env::current_exe()?;
                let directory = image
                    .parent()
                    .ok_or_else(|| std::io::Error::other("Running image has no directory"))?;
                Ok(sugarsubstitute_configuration::layout::InstallationLayout::for_image_directory(
                    directory,
                )
                .runtime_base()
                .to_path_buf())
            }
        }
    }
}
/// Own health through the actual application lifetime and final error resolution.
/// The global subscriber retains bounded cause data and a queue whose admission claim retires with the service.
pub(crate) struct Diagnostics {
    health: DiagnosticHealth,
    #[cfg(windows)]
    service: service::Service,
}
impl Diagnostics {
    /// Keep routing and sink health alive through the caller's final error reporting and native teardown.
    /// Operational history is kept below `records`, the role's installation-local base.
    pub(crate) fn run<T>(
        records: RecordsBase,
        execute: impl FnOnce(DiagnosticHealth) -> Result<T, Box<dyn Error>>,
    ) -> Result<T, Box<dyn Error>> {
        let diagnostics = Self::initialize(records)?;
        let result = execute(diagnostics.health());
        diagnostics.finish(result)
    }
    /// Install the supported process-global subscriber once, retaining two original causes and an omitted count.
    fn initialize(records: RecordsBase) -> Result<Self, Box<dyn Error>> {
        use tracing_subscriber::util::SubscriberInitExt as _;
        let health = DiagnosticHealth::new();
        let filter =
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
        let records = records.resolve()?;
        #[cfg(windows)]
        let service = service::Service::start(
            health.clone(),
            Arc::new(service::AccountSink { base: records }),
            sugarsubstitute_execution::MonotonicClock::new(),
        )?;
        #[cfg(not(windows))]
        drop(records);
        let queue = {
            #[cfg(windows)]
            {
                Some(service.queue())
            }
            #[cfg(not(windows))]
            {
                None
            }
        };
        subscriber(health.clone(), std::io::stderr, filter, queue).try_init()?;
        Ok(Self {
            health,
            #[cfg(windows)]
            service,
        })
    }
    /// Give the actual active host the same data-only diagnostic health owner.
    pub(crate) fn health(&self) -> DiagnosticHealth {
        self.health.clone()
    }
    /// Preserve both the application result and original diagnostic failure at final process observation.
    fn finish<T>(self, result: Result<T, Box<dyn Error>>) -> Result<T, Box<dyn Error>> {
        #[cfg(windows)]
        self.service.finish(&self.health);
        resolve(result, &self.health)
    }
}
/// Resolve independently observed outcomes without replacing either original cause.
pub(crate) fn resolve<T>(
    result: Result<T, Box<dyn Error>>,
    health: &DiagnosticHealth,
) -> Result<T, Box<dyn Error>> {
    match (result, health.check()) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(cause), Ok(())) => Err(cause),
        (Ok(_), Err(cause)) => Err(Box::new(cause)),
        (Err(application), Err(diagnostic)) => {
            Err(Box::new(LifetimeFailure { application, diagnostic }))
        }
    }
}
struct LifetimeFailure {
    application: Box<dyn Error>,
    diagnostic: DiagnosticFailure,
}
impl fmt::Debug for LifetimeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LifetimeFailure")
            .field("diagnostic", &self.diagnostic)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for LifetimeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Application and owned diagnostic output failed")
    }
}
impl Error for LifetimeFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.application.as_ref())
    }
}
#[cfg(test)]
#[path = "diagnostics/tests.rs"]
mod tests;
