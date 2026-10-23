// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep private browser storage jobs alive until native collection retirement or explicit preservation.
use super::*;
use setup_storage::{
    InstallerProfile,
    cache::{CachePolicy, RuntimeCache},
};
use std::time::Duration;
use sugarsubstitute_contracts::execution::ExecutionPort;
use sugarsubstitute_desktop::installer::BrowserRetirement;

#[derive(Default)]
pub(super) struct Profiles {
    requested: bool,
    closing: bool,
    done: bool,
    cache: Option<RuntimeCache>,
    profile: Option<InstallerProfile>,
    observed: Option<PathBuf>,
    retirement: Option<BrowserRetirement>,
    attempted: Vec<String>,
    job: Option<OwnedJob<Outcome, io::Error>>,
}
enum Work {
    Root(PathBuf),
    Prepare(RuntimeCache, Vec<String>),
    Verify(InstallerProfile, PathBuf),
    Finish(InstallerProfile, RuntimeCache, BrowserRetirement),
}
enum Outcome {
    Root(RuntimeCache),
    Prepared(InstallerProfile),
    Verified(InstallerProfile, Option<io::Error>),
    Finished(Option<InstallerProfile>, Option<io::Error>),
}
impl Profiles {
    pub(super) fn request(&mut self) {
        self.requested = true;
    }
    pub(super) fn requested(&self) -> bool {
        self.requested
    }
    pub(super) fn path(&self) -> Option<PathBuf> {
        if self.retirement.is_some() || self.closing {
            return None;
        }
        self.profile.as_ref().map(InstallerProfile::path)
    }
    pub(super) fn ready(&self) -> bool {
        !self.closing && self.profile.as_ref().is_some_and(InstallerProfile::verified)
    }
    pub(super) fn recovering(&self) -> bool {
        self.profile.as_ref().is_some_and(InstallerProfile::recovering)
    }
    pub(super) fn observed(&mut self, path: PathBuf) -> Result<()> {
        if self.closing || self.observed.is_some() || self.profile.is_none() || self.job.is_some() {
            return Err("Browser environment observation is out of order".into());
        }
        self.observed = Some(path);
        Ok(())
    }
    pub(super) fn retired(&mut self, outcome: BrowserRetirement) -> Result<()> {
        if self.retirement.is_some() {
            if self.closing && outcome == BrowserRetirement::NoEffects {
                return Ok(());
            }
            return Err("Browser retirement already observed".into());
        }
        self.retirement = Some(outcome);
        Ok(())
    }
    pub(super) fn close(&mut self) {
        self.closing = true;
    }
    pub(super) fn settled(&self) -> bool {
        !self.requested || self.done
    }
    pub(super) fn advance(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        workspace: &std::path::Path,
    ) -> Result<()> {
        if let Some(job) = &mut self.job
            && let Some(completion) = job.try_take_completion()
        {
            self.job = None;
            match completion.outcome {
                JobOutcome::Success(Outcome::Root(cache)) => self.cache = Some(cache),
                JobOutcome::Success(Outcome::Prepared(profile)) => self.profile = Some(profile),
                JobOutcome::Success(Outcome::Verified(profile, failure)) => {
                    self.profile = Some(profile);
                    if let Some(cause) = failure {
                        return Err(cause.into());
                    }
                }
                JobOutcome::Success(Outcome::Finished(profile, failure)) => {
                    self.profile = profile;
                    if let Some(cause) = failure {
                        return Err(cause.into());
                    }
                    self.observed = None;
                    self.retirement = None;
                    if self.closing {
                        self.done = true;
                    }
                }
                JobOutcome::Failure(cause) => return Err(cause.into()),
                _ => return Err("Browser storage job did not complete".into()),
            }
        }
        if self.job.is_some() || !self.requested || self.done {
            return Ok(());
        }
        if self.closing && self.profile.is_none() {
            self.done = true;
            return Ok(());
        }
        let work = if self.cache.is_none() {
            Work::Root(workspace.to_path_buf())
        } else if self.profile.is_none() && !self.closing {
            Work::Prepare(
                self.cache.as_ref().ok_or("Browser cache missing")?.clone(),
                self.attempted.clone(),
            )
        } else if let Some(outcome) = self.retirement {
            let profile = self.profile.take().ok_or("Browser profile missing")?;
            if profile.recovering() && !self.closing {
                self.attempted.push(profile.generation()?);
            }
            Work::Finish(
                profile,
                self.cache.as_ref().ok_or("Browser cache missing")?.clone(),
                outcome,
            )
        } else if let Some(path) = self.observed.take() {
            Work::Verify(self.profile.take().ok_or("Browser profile missing")?, path)
        } else {
            return Ok(());
        };
        let bytes = if matches!(&work, Work::Root(_)) { 4 } else { 8 } * 1024 * 1024;
        self.job = Some(client.submit_blocking(
            scope,
            JobSpec {
                id: jobs.allocate()?,
                operation: Operation::Infrastructure,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(client.now()?.checked_add(Duration::from_secs(30))?),
                reservation_bytes: bytes,
                replacement: None,
            },
            move |context| match work {
                Work::Root(path) => {
                    RuntimeCache::open(&path, CachePolicy::default(), &context).map(Outcome::Root)
                }
                Work::Prepare(cache, attempted) => {
                    InstallerProfile::prepare(&cache, &attempted, &context).map(Outcome::Prepared)
                }
                Work::Verify(mut profile, path) => {
                    let failure = profile.verify(&path, &context).err();
                    Ok(Outcome::Verified(profile, failure))
                }
                Work::Finish(mut profile, cache, outcome) => {
                    let released = match outcome {
                        BrowserRetirement::Released => true,
                        BrowserRetirement::NoEffects => !profile.recovering(),
                        BrowserRetirement::Unproven => false,
                    };
                    if let Err(cause) = profile.finish(released, &context) {
                        return Ok(Outcome::Finished(Some(profile), Some(cause)));
                    }
                    drop(profile);
                    let failure =
                        cache.maintain(setup_storage::cache::utc_seconds()?, &context).err();
                    Ok(Outcome::Finished(None, failure))
                }
            },
        )?);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closing_during_recovery_finalization_preserves_the_original_release_fact() -> Result<()> {
        let mut profiles = Profiles::default();
        profiles.retired(BrowserRetirement::Released)?;
        profiles.close();
        profiles.retired(BrowserRetirement::NoEffects)?;
        assert_eq!(profiles.retirement, Some(BrowserRetirement::Released));
        assert!(profiles.retired(BrowserRetirement::Released).is_err());
        Ok(())
    }
    #[test]
    fn headless_shutdown_requires_no_browser_profile_or_sdk_observation() {
        let mut profiles = Profiles::default();
        profiles.close();
        assert!(profiles.settled());
        assert!(profiles.path().is_none());
    }
}
