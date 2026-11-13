// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain authenticated role floors even when a later download or verification fails.
use crate::{Error, MAX_METADATA_BYTES, Target, checkpoint::Checkpoint};
use chrono::{DateTime, Utc};
use tuf::{
    database::Database,
    metadata::{
        Metadata, MetadataPath, RawSignedMetadata, RootMetadata, SnapshotMetadata, TargetPath,
        TargetsMetadata, TimestampMetadata,
    },
    pouf::Pouf1,
};

/// Exact unmodified remote role bytes; URLs, filenames and HTTP headers grant no trust.
pub struct MetadataSet<'a> {
    /// Timestamp metadata, verified first.
    pub timestamp: &'a [u8],
    /// Snapshot bytes bound by the timestamp.
    pub snapshot: &'a [u8],
    /// Top-level targets bytes bound by the snapshot.
    pub targets: &'a [u8],
}

/// One repository's accepted root and rollback protection, isolated from other repositories.
///
/// After every rotation or refresh attempt, persist `checkpoint()` under native exclusion,
/// including after errors: an authenticated timestamp may advance before a later role fails.
/// Do not activate or advertise an offer before that durable publication succeeds.
pub struct RepositoryTrust {
    checkpoint: Checkpoint,
    verified: Option<Database<Pouf1>>,
    not_after: i64,
}
impl RepositoryTrust {
    /// Initialize from root metadata embedded in a trusted installer/application distribution.
    /// # Errors
    /// Rejects invalid bounds, malformed roots, missing self-signatures and unsupported policy.
    /// This is an explicit trust boundary: never bootstrap from downloaded root bytes.
    pub fn bootstrap(trusted_root: &[u8]) -> Result<Self, Error> {
        if trusted_root.is_empty() || trusted_root.len() > MAX_METADATA_BYTES {
            return Err(Error::Bound);
        }
        let checkpoint = Checkpoint::new(trusted_root.to_vec());
        Self::from_checkpoint(checkpoint)
    }
    /// Restore authoritative state read from the installation's protected trust store.
    /// # Errors
    /// Rejects unknown or damaged state without falling back to a fresh root or empty floors.
    /// The caller must verify store ownership and filesystem custody before using these bytes;
    /// this method does not authenticate a file supplied by a server or another installation.
    pub fn from_trusted_checkpoint(bytes: &[u8]) -> Result<Self, Error> {
        Self::from_checkpoint(Checkpoint::decode(bytes)?)
    }
    /// Recheck protected trust state without turning cached targets into fresh authorization.
    fn from_checkpoint(checkpoint: Checkpoint) -> Result<Self, Error> {
        let database = database(&checkpoint.root)?;
        if !database.trusted_root().consistent_snapshot() {
            return Err(Error::State);
        }
        Ok(Self { checkpoint, verified: None, not_after: 0 })
    }
    /// Serialize accepted trust for atomic durable publication; it is not disposable cache.
    /// # Errors
    /// Rejects serialization or size failures rather than omitting accepted rollback floors.
    pub fn checkpoint(&self) -> Result<Vec<u8>, Error> {
        self.checkpoint.encode()
    }

    /// Accepted floors for crate-internal decisions that must not reread or mutate trust.
    pub(crate) fn accepted(&self) -> &Checkpoint {
        &self.checkpoint
    }

    /// Versioned root filename to request next, including when the current root has expired.
    /// # Errors
    /// Rejects damaged trusted state or exhausted version space.
    pub fn next_root_version(&self) -> Result<u32, Error> {
        database(&self.checkpoint.root)?.trusted_root().version().checked_add(1).ok_or(Error::Bound)
    }

    /// Version of the currently trusted root. Any accepted rotation increases it, so callers
    /// compare it across a transaction to detect a rotation that kept old signatures valid.
    /// # Errors
    /// Rejects damaged trusted state.
    pub fn root_version(&self) -> Result<u32, Error> {
        Ok(database(&self.checkpoint.root)?.trusted_root().version())
    }

    /// Verify exactly the next root, with both the old and new root signature thresholds.
    /// Expired roots may establish their authenticated successor before metadata refresh.
    /// # Errors
    /// Rejects skipped versions, unauthorized rotation, invalid signatures and oversized input.
    pub fn rotate(&mut self, next_root: &[u8]) -> Result<(), Error> {
        let next = raw::<RootMetadata>(next_root)?;
        let mut database = database(&self.checkpoint.root)?;
        database.update_root(&next)?;
        if !database.trusted_root().consistent_snapshot() {
            return Err(Error::State);
        }
        self.checkpoint.root = next_root.to_vec();
        self.verified = None;
        Ok(())
    }
    /// Authenticate a complete set using supplied Unix UTC seconds, without reading a clock.
    /// # Errors
    /// Rejects stale or inconsistent metadata, changed bytes at an accepted version, expiration,
    /// clock rollback and failed signatures. Already verified floors survive later-role failure.
    pub fn refresh(&mut self, metadata: MetadataSet<'_>, now: i64) -> Result<(), Error> {
        let mut database = self.begin_refresh(metadata.timestamp, now)?;
        self.advance_snapshot(&mut database, metadata.snapshot, now)?;
        let targets = raw::<TargetsMetadata>(metadata.targets)?;
        database.update_targets(&self.time(now)?, &targets)?;
        let targets = database.trusted_targets().ok_or(Error::Incomplete)?;
        self.checkpoint.targets.advance(targets.version(), metadata.targets)?;
        self.not_after = [
            database.trusted_timestamp().ok_or(Error::Incomplete)?.expires().timestamp(),
            database.trusted_root().expires().timestamp(),
            database.trusted_snapshot().ok_or(Error::Incomplete)?.expires().timestamp(),
            targets.expires().timestamp(),
        ]
        .into_iter()
        .min()
        .ok_or(Error::Incomplete)?;
        if now >= self.not_after {
            return Err(Error::Expired);
        }
        self.verified = Some(database);
        Ok(())
    }

    /// Authenticate the snapshot version before using it to request remote bytes.
    /// Persist the checkpoint before requesting that snapshot. This does not authorize targets.
    /// # Errors
    /// Rejects invalid signatures, expiry, clock regression and accepted role/reference rollback.
    pub fn snapshot_version(&mut self, timestamp: &[u8], now: i64) -> Result<u32, Error> {
        let database = self.begin_refresh(timestamp, now)?;
        Ok(database.trusted_timestamp().ok_or(Error::Incomplete)?.snapshot().version())
    }

    /// Authenticate the targets version after fetching the timestamp-bound snapshot.
    /// Persist accepted progress even on failure; a complete refresh still precedes target use.
    /// # Errors
    /// Rejects inconsistent, expired, replayed or altered metadata and reference regression.
    pub fn targets_version(
        &mut self,
        timestamp: &[u8],
        snapshot: &[u8],
        now: i64,
    ) -> Result<u32, Error> {
        let mut database = self.begin_refresh(timestamp, now)?;
        self.advance_snapshot(&mut database, snapshot, now)
    }

    /// Clear earlier target authority and retain the authenticated timestamp and its reference.
    fn begin_refresh(&mut self, bytes: &[u8], now: i64) -> Result<Database<Pouf1>, Error> {
        self.verified = None;
        let time = self.time(now)?;
        let mut database = database(&self.checkpoint.root)?;
        if database.trusted_root().expires() <= &time {
            return Err(Error::Expired);
        }
        let timestamp = raw::<TimestampMetadata>(bytes)?;
        database.update_timestamp(&time, &timestamp)?;
        let timestamp = database.trusted_timestamp().ok_or(Error::Incomplete)?;
        self.checkpoint.snapshot.require(timestamp.snapshot().version())?;
        if timestamp.snapshot().version() < self.checkpoint.minimum_snapshot {
            return Err(Error::Rollback);
        }
        self.checkpoint.timestamp.advance(timestamp.version(), bytes)?;
        self.checkpoint.minimum_snapshot = timestamp.snapshot().version();
        self.checkpoint.time_floor = now;
        Ok(database)
    }

    /// Advance the snapshot only through the authenticated timestamp in this verification.
    fn advance_snapshot(
        &mut self,
        database: &mut Database<Pouf1>,
        bytes: &[u8],
        now: i64,
    ) -> Result<u32, Error> {
        let snapshot = raw::<SnapshotMetadata>(bytes)?;
        database.update_snapshot(&self.time(now)?, &snapshot)?;
        let snapshot = database.trusted_snapshot().ok_or(Error::Incomplete)?;
        let targets_version =
            snapshot.meta().get(&MetadataPath::targets()).ok_or(Error::Incomplete)?.version();
        self.checkpoint.targets.require(targets_version)?;
        if targets_version < self.checkpoint.minimum_targets {
            return Err(Error::Rollback);
        }
        self.checkpoint.snapshot.advance(snapshot.version(), bytes)?;
        self.checkpoint.minimum_targets = targets_version;
        Ok(targets_version)
    }
    /// Issue a bounded target verification ticket from the current complete metadata set.
    /// # Errors
    /// Rejects missing targets, incomplete refreshes, expired trust, unsupported hashes and bounds.
    pub fn target(&self, path: &str, now: i64) -> Result<Target, Error> {
        if path.len() > 1024 {
            return Err(Error::Bound);
        }
        self.time(now)?;
        let database = self.verified.as_ref().ok_or(Error::Incomplete)?;
        if now >= self.not_after {
            return Err(Error::Expired);
        }
        let name = TargetPath::new(path)?;
        let description = database
            .trusted_targets()
            .ok_or(Error::Incomplete)?
            .targets()
            .get(&name)
            .ok_or(Error::Incomplete)?;
        Target::new(name.as_str(), description, self.checkpoint.time_floor, self.not_after)
    }
    /// Keep clock regression from extending metadata validity across restarts.
    fn time(&self, now: i64) -> Result<DateTime<Utc>, Error> {
        if now < self.checkpoint.time_floor {
            return Err(Error::ClockRollback);
        }
        DateTime::from_timestamp(now, 0).ok_or(Error::Bound)
    }
}

/// Bound untrusted role bytes before the TUF parser allocates its metadata representation.
fn raw<M: Metadata>(bytes: &[u8]) -> Result<RawSignedMetadata<Pouf1, M>, Error> {
    if bytes.is_empty() || bytes.len() > MAX_METADATA_BYTES {
        return Err(Error::Bound);
    }
    Ok(RawSignedMetadata::new(bytes.to_vec()))
}
/// Construct verification state only from the caller's out-of-band or protected trusted root.
fn database(root: &[u8]) -> Result<Database<Pouf1>, Error> {
    Ok(Database::from_trusted_root(&raw::<RootMetadata>(root)?)?)
}
