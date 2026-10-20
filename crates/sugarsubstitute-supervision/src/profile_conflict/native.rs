// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify the profile, its coordinator namespace and original kernel process objects.
use super::*;
use crate::{
    ProtectedStorageRoot,
    native_claims::{
        Record, RecordState,
        files::{self, Budget, Handle},
    },
    protected_storage::{
        account,
        exclusion::{ExclusionOpen, HeldFile},
        native_name::{NativeName, NativeSid, NativeSidDestination},
        native_phase::NativePhase,
        record_file::{self, RecordRead},
    },
};
use processes::Process;
use windows::Win32::{
    Foundation::*,
    Security::{GetTokenInformation, TOKEN_QUERY, TokenSessionId},
    System::Threading::*,
};

pub(super) struct Native {
    request: ConflictRequest,
    memory: Option<Arc<sugarsubstitute_contracts::execution::MemoryClaim>>,
    root: Option<ProtectedStorageRoot>,
    writer: Option<HeldFile<160>>,
    custody: Option<HeldFile<160>>,
    coordinator: Option<HeldFile<128>>,
    handles: Vec<Handle>,
    processes: Vec<Process>,
    resources: resources::Resources,
    original: Option<[u8; 160]>,
    target: Option<usize>,
    retired: bool,
}
impl Native {
    pub(super) fn new(request: ConflictRequest) -> Self {
        Self {
            request,
            memory: None,
            root: None,
            writer: None,
            custody: None,
            coordinator: None,
            handles: Vec::new(),
            processes: Vec::new(),
            resources: Default::default(),
            original: None,
            target: None,
            retired: false,
        }
    }
    pub(super) fn execute(
        &mut self,
        operation: ConflictOperation,
        context: &JobContext,
    ) -> Result<ConflictObservation, ClaimError> {
        check(context)?;
        match operation {
            ConflictOperation::Retire => {
                self.retire(context)?;
                Ok(ConflictObservation::Retired)
            }
            _ if self.retired => Err(ClaimError::invalid("profile_conflict_retired")),
            ConflictOperation::Discover => self.discover(context),
            ConflictOperation::Observe => self.observe(context),
            ConflictOperation::Close | ConflictOperation::Force => {
                if self.observe(context)? == ConflictObservation::Released {
                    return Ok(ConflictObservation::Released);
                }
                self.validate_owner(context)?;
                let index =
                    self.target.ok_or_else(|| ClaimError::invalid("profile_conflict_target"))?;
                let budget = self.budget()?;
                processes::family(index, &mut self.processes, budget, &mut self.handles)?;
                check(context)?;
                if operation == ConflictOperation::Force {
                    self.processes[index].force()?;
                } else {
                    processes::close_windows(
                        &self.processes[index..],
                        &self.processes[index].image,
                    )?;
                }
                Ok(ConflictObservation::Waiting)
            }
        }
    }
    fn budget(&self) -> Result<Budget, ClaimError> {
        Ok(Budget::Protected(
            self.memory
                .as_ref()
                .ok_or_else(|| ClaimError::invalid("profile_conflict_memory"))?
                .clone(),
        ))
    }
    fn discover(&mut self, context: &JobContext) -> Result<ConflictObservation, ClaimError> {
        if self.root.is_some() {
            return Err(ClaimError::invalid("profile_conflict_repeated_discovery"));
        }
        self.memory = Some(Arc::new(
            context
                .memory()
                .claim(2 * 1024 * 1024)
                .map_err(|cause| ClaimError::new("profile_conflict_memory", cause))?,
        ));
        self.root = Some(ProtectedStorageRoot::new(
            context
                .memory()
                .claim(1024 * 1024)
                .map_err(|cause| ClaimError::new("profile_conflict_root_memory", cause))?,
        )?);
        let root =
            self.root.as_mut().ok_or_else(|| ClaimError::invalid("profile_conflict_root"))?;
        root.resolve_or_create_profile_leaf(&self.request.path, context)?;
        let (folder, identity, sid) = root.profile_folder()?;
        if identity != self.request.folder
            || crate::profiles::record::account_identity(sid) != self.request.account
        {
            return Err(ClaimError::invalid("profile_conflict_changed_folder"));
        }
        let (folder, sid) = (folder.to_path_buf(), sid.to_vec());
        let budget = self.budget()?;
        let acquired = open(
            &mut self.writer,
            &folder,
            ".sugarsubstitute-profile-v1.claim",
            &sid,
            budget.clone(),
        )?;
        if acquired {
            return self.observe(context);
        }
        let bytes = self.read(context)?;
        let row = crate::profiles::record::custody::CustodiedRecord::decode(&bytes)?;
        if row.original.state != crate::profiles::record::State::Held
            || row.original.folder != identity
            || row.original.account != self.request.account
        {
            return Err(ClaimError::invalid("profile_conflict_record_identity"));
        }
        self.original = Some(bytes);
        let users = self.resources.users(&folder.join(".sugarsubstitute-profile-v1.claim"))?;
        for user in users {
            if user.dwProcessId == std::process::id() {
                continue;
            }
            check(context)?;
            let index = Process::open(user.dwProcessId, true, budget.clone(), &mut self.processes)?;
            let process = &self.processes[index];
            if !process.matches(&user)
                || process.exited()?
                || process.image.file_name() != Some(std::ffi::OsStr::new("sugarsubstitute.exe"))
            {
                self.processes[index].handle.close()?;
                self.processes.pop();
                continue;
            }
            let installation = process
                .image
                .parent()
                .ok_or_else(|| ClaimError::invalid("profile_conflict_installation"))?
                .to_path_buf();
            let namespace =
                account::resolve_namespace(&installation, &mut self.handles, budget.clone())?;
            if namespace.namespace != row.original.run.owner.namespace {
                return Err(ClaimError::invalid("profile_conflict_namespace"));
            }
            let mut token = HANDLE::default();
            // SAFETY: The retained target process and token output remain valid; adopt before inspection.
            let opened = unsafe {
                OpenProcessToken(self.processes[index].handle.raw(), TOKEN_QUERY, &mut token)
            };
            self.handles.push(Handle::new(token, budget.clone()));
            opened.map_err(|cause| ClaimError::new("profile_conflict_token", cause))?;
            let actual_sid = account::token_sid(
                token,
                &mut NativePhase::ordinary(&budget)?,
                NativeSidDestination::Ordinary,
            )?;
            let mut session = 0_u32;
            let mut returned = 0;
            // SAFETY: The retained process prevents PID reuse during this session query.
            unsafe {
                GetTokenInformation(
                    token,
                    TokenSessionId,
                    Some((&raw mut session).cast()),
                    4,
                    &mut returned,
                )
            }
            .map_err(|cause| ClaimError::new("profile_conflict_session_identity", cause))?;
            if actual_sid.bytes()? != namespace.account_sid || session != namespace.session_id {
                return Err(ClaimError::invalid("profile_conflict_account"));
            }
            let stem = namespace
                .directory
                .join("coordinators")
                .join(record_file::hex(namespace.namespace.bytes()));
            let claim = stem.with_extension("claim");
            let users = self.resources.users(&claim)?;
            if !users.iter().any(|user| self.processes[index].matches(user)) {
                return Err(ClaimError::invalid("profile_conflict_coordinator_owner"));
            }
            let result = HeldFile::acquire_into(
                ExclusionOpen::InspectExisting,
                &mut self.coordinator,
                NativeName::Ordinary(claim),
                NativeName::Ordinary(stem.with_extension("record")),
                NativeSid::Ordinary(sid.clone()),
                budget.clone(),
                &mut NativePhase::ordinary(&budget)?,
            );
            require_contention(result)?;
            let file = self
                .coordinator
                .as_ref()
                .ok_or_else(|| ClaimError::invalid("profile_conflict_coordinator"))?;
            let RecordRead::Present(bytes) = record_file::read_shape(file, context)? else {
                return Err(ClaimError::invalid("profile_conflict_coordinator_record"));
            };
            let coordinator = Record::decode(&bytes)?;
            if coordinator.owner != row.original.run.owner
                || coordinator.state != RecordState::Held
                || coordinator.resource != 0
            {
                return Err(ClaimError::invalid("profile_conflict_coordinator_epoch"));
            }
            self.target = Some(index);
            processes::family(index, &mut self.processes, budget, &mut self.handles)?;
            self.validate_owner(context)?;
            return Ok(ConflictObservation::Owner(installation));
        }
        Err(ClaimError::invalid("profile_conflict_owner_unavailable"))
    }
    fn read(&self, context: &JobContext) -> Result<[u8; 160], ClaimError> {
        let writer =
            self.writer.as_ref().ok_or_else(|| ClaimError::invalid("profile_conflict_writer"))?;
        match record_file::read_shape(writer, context)? {
            RecordRead::Present(bytes) => Ok(bytes),
            _ => Err(ClaimError::invalid("profile_conflict_missing_record")),
        }
    }
    fn validate_owner(&mut self, context: &JobContext) -> Result<(), ClaimError> {
        check(context)?;
        let index = self.target.ok_or_else(|| ClaimError::invalid("profile_conflict_target"))?;
        if self.processes[index].exited()? || self.original != Some(self.read(context)?) {
            return Err(ClaimError::invalid("profile_conflict_owner_changed"));
        }
        let users =
            self.resources.users(&self.request.path.join(".sugarsubstitute-profile-v1.claim"))?;
        if !users.iter().any(|user| self.processes[index].matches(user)) {
            return Err(ClaimError::invalid("profile_conflict_owner_changed"));
        }
        check(context)
    }
    fn observe(&mut self, context: &JobContext) -> Result<ConflictObservation, ClaimError> {
        for process in &self.processes {
            if !process.exited()? {
                return Ok(ConflictObservation::Waiting);
            }
        }
        if let Some(file) = self.writer.as_mut() {
            file.close()?;
        }
        self.writer = None;
        let root =
            self.root.as_ref().ok_or_else(|| ClaimError::invalid("profile_conflict_root"))?;
        let (folder, _, sid) = root.profile_folder()?;
        let (folder, sid) = (folder.to_path_buf(), sid.to_vec());
        let budget = self.budget()?;
        if !open(
            &mut self.writer,
            &folder,
            ".sugarsubstitute-profile-v1.claim",
            &sid,
            budget.clone(),
        )? {
            return Ok(ConflictObservation::Waiting);
        }
        if let Some(file) = self.custody.as_mut() {
            file.close()?;
        }
        self.custody = None;
        check(context)?;
        if !open(&mut self.custody, &folder, ".sugarsubstitute-profile-v2.custody", &sid, budget)? {
            return Ok(ConflictObservation::Waiting);
        }
        Ok(ConflictObservation::Released)
    }
    fn retire(&mut self, context: &JobContext) -> Result<(), ClaimError> {
        self.resources.close()?;
        for file in [&mut self.writer, &mut self.custody] {
            if let Some(value) = file {
                value.close()?;
            }
            *file = None;
        }
        if let Some(file) = self.coordinator.as_mut() {
            file.close()?;
        }
        self.coordinator = None;
        for process in &mut self.processes {
            process.handle.close()?;
        }
        self.processes.clear();
        files::close_all(&mut self.handles)?;
        if let Some(root) = self.root.as_mut() {
            root.retire(context)?;
        }
        self.root = None;
        self.memory = None;
        self.retired = true;
        Ok(())
    }
}
fn open(
    slot: &mut Option<HeldFile<160>>,
    folder: &std::path::Path,
    name: &str,
    sid: &[u8],
    budget: Budget,
) -> Result<bool, ClaimError> {
    match HeldFile::acquire_into(
        ExclusionOpen::InspectExisting,
        slot,
        NativeName::Ordinary(folder.join(name)),
        NativeName::Ordinary(folder.join(".sugarsubstitute-profile-v1.record")),
        NativeSid::Ordinary(sid.to_vec()),
        budget.clone(),
        &mut NativePhase::ordinary(&budget)?,
    ) {
        Ok(()) => Ok(true),
        Err(cause) if contention(&cause) => Ok(false),
        Err(cause) => Err(cause),
    }
}
fn contention(cause: &ClaimError) -> bool {
    use std::error::Error;
    cause.operation() == "claim_exclusion"
        && cause.cleanup_cause().is_none()
        && matches!(
            cause.source().and_then(|cause| cause.downcast_ref::<std::fs::TryLockError>()),
            Some(std::fs::TryLockError::WouldBlock)
        )
}
fn require_contention(result: Result<(), ClaimError>) -> Result<(), ClaimError> {
    match result {
        Err(cause) if contention(&cause) => Ok(()),
        Err(cause) => Err(cause),
        Ok(()) => Err(ClaimError::invalid("profile_conflict_coordinator_unowned")),
    }
}
