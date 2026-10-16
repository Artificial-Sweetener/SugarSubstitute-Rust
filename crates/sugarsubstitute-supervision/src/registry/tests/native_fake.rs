// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Controlled external native effects; registry policy and execution remain real.
use super::*;
#[derive(Default)]
pub(super) struct Fake {
    pub(super) creates: AtomicUsize,
    pub(super) resumes: AtomicUsize,
    pub(super) seals: AtomicUsize,
    pub(super) forces: AtomicUsize,
    pub(super) closes: AtomicUsize,
    pub(super) fail_seal: AtomicBool,
    pub(super) fail_inventory: AtomicBool,
    pub(super) fail_close: AtomicBool,
    pub(super) fail_close_after_job: AtomicBool,
    pub(super) fail_resume_close: AtomicBool,
    pub(super) fail_duplicate: AtomicBool,
    pub(super) partial_duplicate_failure: AtomicBool,
    pub(super) duplicates: AtomicUsize,
    pub(super) fail_enrollment: AtomicBool,
    pub(super) hold_enrollment: AtomicBool,
    pub(super) enrollment_entered: AtomicBool,
    pub(super) creation_order: Mutex<[u8; 4]>,
    pub(super) inherited: Mutex<([usize; 6], usize, [usize; 3], bool)>,
    pub(super) fail_terminate: AtomicBool,
    pub(super) advance_clock: Mutex<Option<Arc<TestClock>>>,
    pub(super) root_alive: AtomicBool,
    pub(super) leaf_alive: AtomicBool,
    pub(super) include_leaf: AtomicBool,
    pub(super) hold_create: AtomicBool,
    pub(super) panic_create: AtomicBool,
    pub(super) state: Mutex<(bool, bool)>,
    pub(super) changed: Condvar,
}
impl Fake {
    pub(super) fn entered_enrollment(&self) -> Result<(), Box<dyn Error>> {
        let guard = lock(&self.state);
        let (_guard, timed) = self
            .changed
            .wait_timeout_while(guard, Duration::from_secs(10), |_| {
                !self.enrollment_entered.load(Ordering::Acquire)
            })
            .map_err(|_| "Native enrollment fixture poisoned")?;
        if timed.timed_out() {
            return Err("Native enrollment boundary was not entered".into());
        }
        Ok(())
    }
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self { root_alive: AtomicBool::new(true), ..Self::default() })
    }
    pub(super) fn release(&self) {
        lock(&self.state).1 = true;
        self.changed.notify_all();
    }
    pub(super) fn entered(&self) -> Result<(), Box<dyn Error>> {
        let guard = lock(&self.state);
        let (_guard, timed) = self
            .changed
            .wait_timeout_while(guard, Duration::from_secs(10), |state| !state.0)
            .map_err(|_| "Native fixture poisoned")?;
        if timed.timed_out() {
            return Err("Creation boundary was not entered".into());
        }
        Ok(())
    }
    pub(super) fn closed(&self) -> Result<(), Box<dyn Error>> {
        let guard = lock(&self.state);
        let (_guard, timed) = self
            .changed
            .wait_timeout_while(guard, Duration::from_secs(10), |_| {
                self.closes.load(Ordering::Acquire) == 0
            })
            .map_err(|_| "Native fixture poisoned")?;
        if timed.timed_out() {
            return Err("Owned containment was not closed".into());
        }
        Ok(())
    }
}
pub(super) const ROOT: Identity = Identity { pid: 123, birth: 456 };
pub(super) const LEAF: Identity = Identity { pid: 124, birth: 457 };
impl NativePort for Fake {
    fn prepare_job(&self, family: &mut Family) -> Result<(), RegistryError> {
        if family.job.is_some() {
            return Err(RegistryError::invalid("fake_prepared_job_already_owned"));
        }
        family.job = Some(10);
        Ok(())
    }
    fn create(
        &self,
        family: &mut Family,
        _: &mut WindowsLaunchInput,
    ) -> Result<Identity, RegistryError> {
        self.creates.fetch_add(1, Ordering::AcqRel);
        *lock(&self.creation_order) = [1, 0, 0, 0];
        family.job = Some(10);
        family.thread = Some(11);
        family.members[0] =
            Some(native_process::Member { handle: Some(12), identity: ROOT, exited: false });
        family.root = Some(ROOT);
        family.count = 1;
        let mut startup = windows::Win32::System::Threading::STARTUPINFOW::default();
        family.configure_stdio(&mut startup);
        *lock(&self.inherited) = (
            family.inherited_handles,
            family.inherited_count,
            [
                startup.hStdInput.0 as usize,
                startup.hStdOutput.0 as usize,
                startup.hStdError.0 as usize,
            ],
            startup.dwFlags.contains(windows::Win32::System::Threading::STARTF_USESTDHANDLES),
        );
        family.attributes_initialized = true;
        if self.panic_create.load(Ordering::Acquire) {
            std::panic::resume_unwind(Box::new("private native creation owner payload"));
        }
        let mut guard = lock(&self.state);
        guard.0 = true;
        self.changed.notify_all();
        if self.hold_create.load(Ordering::Acquire) {
            let (_guard, timed) = self
                .changed
                .wait_timeout_while(guard, Duration::from_secs(10), |state| !state.1)
                .map_err(|_| RegistryError::invalid("fake_creation_latch"))?;
            if timed.timed_out() {
                return Err(RegistryError::invalid("fake_creation_latch"));
            }
        }
        Ok(ROOT)
    }
    fn resume(&self, family: &mut Family) -> Result<(), RegistryError> {
        lock(&self.creation_order)[3] = 4;
        self.resumes.fetch_add(1, Ordering::AcqRel);
        family.resumed = true;
        if self.fail_resume_close.load(Ordering::Acquire) {
            return Err(RegistryError::new(
                "close_primary_thread",
                std::io::Error::from_raw_os_error(6),
            ));
        }
        family.thread = None;
        family.attributes_initialized = false;
        family.release_inheritance()?;
        Ok(())
    }
    fn duplicate(&self, family: &mut Family, _: usize) -> Result<WatchInput, RegistryError> {
        self.duplicates.fetch_add(1, Ordering::AcqRel);
        if self.partial_duplicate_failure.load(Ordering::Acquire) {
            family.watch_duplicate = Some(14);
            return Err(RegistryError::new(
                "duplicate_watch_process",
                std::io::Error::from_raw_os_error(6),
            ));
        }
        if self.fail_duplicate.load(Ordering::Acquire) {
            return Err(RegistryError::new(
                "duplicate_watch_process",
                std::io::Error::from_raw_os_error(6),
            ));
        }
        lock(&self.creation_order)[1] = 2;
        Ok(WatchInput::Simulated)
    }
    fn enroll_desktop(
        &self,
        _: &mut Family,
        run: sugarsubstitute_contracts::lifecycle::RunKey,
        key: ProcessKey,
    ) -> Result<(), RegistryError> {
        if key.owner != run.owner || *lock(&self.creation_order) != [1, 2, 0, 0] {
            return Err(RegistryError::invalid("fake_enrollment_order"));
        }
        self.enrollment()
    }
    fn enroll_diagnostic(
        &self,
        family: &mut Family,
        binding: sugarsubstitute_contracts::incident::DiagnosticKey,
        key: ProcessKey,
    ) -> Result<(), RegistryError> {
        if key.owner != binding.owner()
            || family.diagnostic_key != Some(binding)
            || family.resumed
            || *lock(&self.creation_order) != [1, 2, 0, 0]
            || family
                .diagnostic_bootstrap
                .as_ref()
                .zip(family.diagnostic_enrollment.as_ref())
                .is_none_or(|(bootstrap, enrollment)| !bootstrap.matches_enrollment(enrollment))
        {
            return Err(RegistryError::invalid("fake_diagnostic_enrollment_order"));
        }
        self.enrollment()
    }
    fn enroll_backend_custody(
        &self,
        family: &mut Family,
        binding: sugarsubstitute_contracts::backend_custody::BackendCustodyKey,
        key: ProcessKey,
    ) -> Result<(), RegistryError> {
        if family.root.is_none()
            || family.resumed
            || key.owner != binding.owner()
            || family.backend_custody.as_ref().is_none_or(|launch| !launch.validates(binding))
        {
            return Err(RegistryError::invalid("fake_custody_enrollment_order"));
        }
        if let Some(launch) = &mut family.backend_custody {
            drop(launch.enrollment.take());
        }
        Ok(())
    }
    fn seal(&self, family: &mut Family) -> Result<(), RegistryError> {
        self.seals.fetch_add(1, Ordering::AcqRel);
        if self.fail_seal.load(Ordering::Acquire) {
            family.incomplete = true;
            return Err(RegistryError::new(
                "seal_job_limits",
                std::io::Error::from_raw_os_error(5),
            ));
        }
        family.sealed = true;
        Ok(())
    }
    fn inventory(&self, family: &mut Family) -> Result<native_process::Inventory, RegistryError> {
        if let Some(clock) = &*lock(&self.advance_clock) {
            clock.0.store(10_000_000_000, Ordering::Release);
        }
        if self.fail_inventory.load(Ordering::Acquire) {
            family.incomplete = true;
            return Err(RegistryError::invalid("complete_job_inventory"));
        }
        if self.include_leaf.load(Ordering::Acquire) && family.count == 1 {
            family.members[1] =
                Some(native_process::Member { handle: Some(13), identity: LEAF, exited: false });
            family.count = 2;
        }
        // Exact external Job query bytes are independent of retained historical members.
        family.query.fill(0);
        let mut pids = [0usize; 2];
        let mut count = 0;
        if self.root_alive.load(Ordering::Acquire) {
            pids[count] = ROOT.pid as usize;
            count += 1;
        }
        if self.include_leaf.load(Ordering::Acquire) && self.leaf_alive.load(Ordering::Acquire) {
            pids[count] = LEAF.pid as usize;
            count += 1;
        }
        let header = [(count as u32).to_ne_bytes(), (count as u32).to_ne_bytes()].concat();
        for (index, chunk) in header.chunks_exact(size_of::<usize>()).enumerate() {
            family.query[index] = usize::from_ne_bytes(std::array::from_fn(|at| chunk[at]));
        }
        let start = 8 / size_of::<usize>();
        family.query[start..start + count].copy_from_slice(&pids[..count]);
        Ok(native_process::Inventory {
            empty: !self.root_alive.load(Ordering::Acquire)
                && !self.leaf_alive.load(Ordering::Acquire),
            count: family.count,
        })
    }
    fn terminate(&self, family: &mut Family) -> Result<(), RegistryError> {
        self.forces.fetch_add(1, Ordering::AcqRel);
        if self.fail_terminate.load(Ordering::Acquire) {
            return Err(RegistryError::new("terminate_job", std::io::Error::from_raw_os_error(5)));
        }
        family.forced = true;
        Ok(())
    }
    fn close(&self, family: &mut Family) -> Result<(), RegistryError> {
        family.attributes_initialized = false;
        family.release_inheritance()?;
        if self.fail_close_after_job.load(Ordering::Acquire) && family.job.take().is_some() {
            return Err(RegistryError::new(
                "close_primary_thread",
                std::io::Error::from_raw_os_error(6),
            ));
        }
        if self.fail_close.load(Ordering::Acquire) {
            return Err(RegistryError::new("close_job", std::io::Error::from_raw_os_error(6)));
        }
        let _guard = lock(&self.state);
        self.closes.fetch_add(1, Ordering::AcqRel);
        self.changed.notify_all();
        family.job = None;
        family.thread = None;
        family.watch_duplicate = None;
        drop(family.diagnostic_enrollment.take());
        for member in family.members.iter_mut().flatten() {
            member.handle = None;
        }
        Ok(())
    }
}

impl Fake {
    /// Hold only the external enrollment effect after real registry ownership/order checks.
    fn enrollment(&self) -> Result<(), RegistryError> {
        lock(&self.creation_order)[2] = 3;
        let guard = lock(&self.state);
        self.enrollment_entered.store(true, Ordering::Release);
        self.changed.notify_all();
        if self.hold_enrollment.load(Ordering::Acquire) {
            let (_guard, timed) = self
                .changed
                .wait_timeout_while(guard, Duration::from_secs(10), |state| !state.1)
                .map_err(|_| RegistryError::invalid("fake_enrollment_latch"))?;
            if timed.timed_out() {
                return Err(RegistryError::invalid("fake_enrollment_latch"));
            }
        }
        if self.fail_enrollment.load(Ordering::Acquire) {
            return Err(RegistryError::new(
                "lifecycle_enrollment_rejected",
                std::io::Error::from_raw_os_error(6),
            ));
        }
        Ok(())
    }
}
