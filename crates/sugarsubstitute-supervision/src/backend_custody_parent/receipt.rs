// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Store receipt coordinates beside their existing exact frame owner without duplicating it.
use super::*;
use sugarsubstitute_contracts::lifecycle::{
    CoordinatorKey, ResourceGeneration, ResourceId, ResourceKey,
};
/// Data only; the original response owner separately validates native source and nonce.
#[derive(Clone, Copy)]
pub(super) enum StoredReceipt {
    Established,
    Finished,
    Prepared(ResourceId, ResourceGeneration),
    Root(ResourceId, ResourceGeneration),
    Released(ResourceId, ResourceGeneration),
    ProfilePrepared(ProfileCustodyIdentity),
    ProfilePublished(ProfileCustodyIdentity),
    ProfileRoot(ProfileCustodyIdentity),
    ProfileReleased(ProfileCustodyIdentity),
}
impl StoredReceipt {
    /// Preserve the complete validated owner before projecting only redundant coordinates.
    pub(super) fn new(
        owner: CoordinatorKey,
        value: BackendCustodyReceipt,
    ) -> Result<Self, NativeObservationError> {
        use BackendCustodyReceipt as R;
        let actual = match value {
            R::Established => return Ok(Self::Established),
            R::Finished => return Ok(Self::Finished),
            R::Prepared(key) | R::Root(key) | R::Released(key) => key.owner,
            R::ProfilePrepared(key)
            | R::ProfilePublished(key)
            | R::ProfileRoot(key)
            | R::ProfileReleased(key) => key.run().owner,
        };
        if actual != owner {
            return Err(reject("custody_receipt_stored_owner"));
        }
        Ok(match value {
            R::Established => Self::Established,
            R::Finished => Self::Finished,
            R::Prepared(key) => Self::Prepared(key.id, key.generation),
            R::Root(key) => Self::Root(key.id, key.generation),
            R::Released(key) => Self::Released(key.id, key.generation),
            R::ProfilePrepared(key) => Self::ProfilePrepared(key.identity()),
            R::ProfilePublished(key) => Self::ProfilePublished(key.identity()),
            R::ProfileRoot(key) => Self::ProfileRoot(key.identity()),
            R::ProfileReleased(key) => Self::ProfileReleased(key.identity()),
        })
    }
    /// Return the identical public complete receipt with its unchanged retained frame owner.
    pub(super) const fn expand(self, owner: CoordinatorKey) -> BackendCustodyReceipt {
        use BackendCustodyReceipt as R;
        match self {
            Self::Established => R::Established,
            Self::Finished => R::Finished,
            Self::Prepared(id, generation) => R::Prepared(ResourceKey { owner, id, generation }),
            Self::Root(id, generation) => R::Root(ResourceKey { owner, id, generation }),
            Self::Released(id, generation) => R::Released(ResourceKey { owner, id, generation }),
            Self::ProfilePrepared(key) => R::ProfilePrepared(key.key(owner)),
            Self::ProfilePublished(key) => R::ProfilePublished(key.key(owner)),
            Self::ProfileRoot(key) => R::ProfileRoot(key.key(owner)),
            Self::ProfileReleased(key) => R::ProfileReleased(key.key(owner)),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_contracts::lifecycle::{
        CoordinatorEpoch, InstanceId, NamespaceId, RunGeneration, RunKey,
    };
    #[test]
    fn compact_receipt_facts_preserve_every_complete_identity_and_reject_foreign_owner()
    -> Result<(), Box<dyn std::error::Error>> {
        let owner = CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        };
        let resource =
            ResourceKey { owner, id: ResourceId::new(3)?, generation: ResourceGeneration::new(4)? };
        let profile = ProfileCustodyKey::new(
            RunKey { owner, instance: InstanceId::new(5)?, generation: RunGeneration::new(6)? },
            7,
        )?;
        let foreign = CoordinatorKey { namespace: NamespaceId::new([8; 32]), ..owner };
        use BackendCustodyReceipt as R;
        for receipt in [
            R::Prepared(resource),
            R::Root(resource),
            R::Released(resource),
            R::ProfilePrepared(profile),
            R::ProfilePublished(profile),
            R::ProfileRoot(profile),
            R::ProfileReleased(profile),
        ] {
            assert_eq!(StoredReceipt::new(owner, receipt)?.expand(owner), receipt);
            assert!(StoredReceipt::new(foreign, receipt).is_err());
        }
        for receipt in [R::Established, R::Finished] {
            assert_eq!(StoredReceipt::new(owner, receipt)?.expand(owner), receipt);
        }
        if cfg!(target_pointer_width = "64") {
            assert_eq!(size_of::<StoredReceipt>(), 32);
            assert_eq!(size_of::<BackendCustodyAcknowledgement>(), 112);
        }
        Ok(())
    }
}
