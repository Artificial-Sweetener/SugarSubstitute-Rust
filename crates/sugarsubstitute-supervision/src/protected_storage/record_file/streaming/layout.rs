// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Derive bounded diagnostic leaf names from complete typed identities.
use super::*;
use sugarsubstitute_contracts::incident::{AttachmentKind, IncidentKey, IncidentSubject};

fn coordinator(hash: &mut Sha256, owner: sugarsubstitute_contracts::lifecycle::CoordinatorKey) {
    hash.update(owner.namespace.bytes());
    hash.update(owner.epoch.bytes());
}
pub(crate) fn subject(key: IncidentKey) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"sugarsubstitute-incident-subject-v1\0");
    coordinator(&mut hash, key.subject().coordinator());
    match key.subject() {
        IncidentSubject::Desktop(run) => {
            hash.update([1]);
            hash.update(run.instance.get().to_le_bytes());
            hash.update(run.generation.get().to_le_bytes());
        }
        IncidentSubject::Supervisor(_) => hash.update([2]),
    }
    hash.finalize().into()
}
pub(super) fn operation(key: LogicalStoreOperationKey) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"sugarsubstitute-incident-operation-v1\0");
    coordinator(&mut hash, key.owner());
    hash.update(key.id().get().to_le_bytes());
    hash.finalize().into()
}
pub(crate) fn binding(
    object: DiagnosticObject,
    namespace: NamespaceId,
) -> (DiagnosticEntryClass, [u8; 32]) {
    match object {
        DiagnosticObject::Seed(key) => (DiagnosticEntryClass::Seed, subject(key)),
        DiagnosticObject::Summary(key) => (DiagnosticEntryClass::Summary, subject(key)),
        DiagnosticObject::StoreState => (DiagnosticEntryClass::StoreState, *namespace.bytes()),
        DiagnosticObject::Journal(key) => (DiagnosticEntryClass::Journal, operation(key)),
        DiagnosticObject::Payload(key, ..) => (DiagnosticEntryClass::Payload, subject(key)),
    }
}
pub(super) fn leaf(object: DiagnosticObject, namespace: NamespaceId) -> String {
    match object {
        DiagnosticObject::Seed(key) => format!("{}.seed", super::super::hex(&subject(key))),
        DiagnosticObject::Summary(key) => format!("{}.summary", super::super::hex(&subject(key))),
        DiagnosticObject::StoreState => format!("{}.state", super::super::hex(namespace.bytes())),
        DiagnosticObject::Journal(key) => format!("{}.journal", super::super::hex(&operation(key))),
        DiagnosticObject::Payload(key, revision, kind) => {
            let kind = match kind {
                AttachmentKind::Startup => 1,
                AttachmentKind::Fault => 2,
                AttachmentKind::Runtime => 3,
                AttachmentKind::Log => 4,
                AttachmentKind::SymbolizedFault => 5,
                AttachmentKind::Minidump => 6,
            };
            format!("{}.p-{:016x}-{kind}.payload", super::super::hex(&subject(key)), revision.get())
        }
    }
}
pub(super) fn decode_digest(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64
        || !text.bytes().all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
    {
        return None;
    }
    let mut digest = [0; 32];
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        let value = |byte: u8| if byte <= b'9' { byte - b'0' } else { byte - b'a' + 10 };
        digest[index] = value(pair[0]) * 16 + value(pair[1]);
    }
    (digest != [0; 32]).then_some(digest)
}
#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_contracts::{
        incident::IncidentOperationId,
        lifecycle::{CoordinatorEpoch, CoordinatorKey},
    };
    #[test]
    fn diagnostic_journal_names_distinguish_historical_coordinator_operation_one()
    -> Result<(), Box<dyn std::error::Error>> {
        let namespace = NamespaceId::new([1; 32]);
        let owner = |epoch| CoordinatorKey { namespace, epoch };
        let first = LogicalStoreOperationKey::new(
            owner(CoordinatorEpoch::new([2; 16])?),
            IncidentOperationId::new(1)?,
        );
        let later = LogicalStoreOperationKey::new(
            owner(CoordinatorEpoch::new([3; 16])?),
            IncidentOperationId::new(1)?,
        );
        assert_ne!(
            leaf(DiagnosticObject::Journal(first), namespace),
            leaf(DiagnosticObject::Journal(later), namespace)
        );
        assert_ne!(
            leaf(
                DiagnosticObject::Seed(IncidentKey::new(IncidentSubject::Supervisor(
                    first.owner()
                ))),
                namespace
            ),
            leaf(
                DiagnosticObject::Seed(IncidentKey::new(IncidentSubject::Supervisor(
                    later.owner()
                ))),
                namespace
            )
        );
        Ok(())
    }
}
