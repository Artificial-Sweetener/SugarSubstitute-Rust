// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Derive bounded diagnostic leaf names from complete typed identities.
use super::*;
use sugarsubstitute_contracts::incident::{
    AttachmentKind, IncidentKey, IncidentRevision, IncidentSubject,
};

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
/// Count one immutable payload generation independently of its attachment kinds.
pub(super) fn payload_group(digest: [u8; 32], revision: IncidentRevision) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"sugarsubstitute-incident-payload-group-v1\0");
    hash.update(digest);
    hash.update(revision.get().to_le_bytes());
    hash.finalize().into()
}
/// Project physical accounting data without changing the object's subject binding.
pub(crate) fn object_payload_group(object: DiagnosticObject) -> Option<[u8; 32]> {
    match object {
        DiagnosticObject::Payload(key, revision, _) => Some(payload_group(subject(key), revision)),
        _ => None,
    }
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
pub(super) fn write_leaf(
    object: DiagnosticObject,
    namespace: NamespaceId,
    output: &mut impl std::fmt::Write,
) -> std::fmt::Result {
    use super::leaf::Hex;
    match object {
        DiagnosticObject::Seed(key) => write!(output, "{}.seed", Hex(&subject(key))),
        DiagnosticObject::Summary(key) => write!(output, "{}.summary", Hex(&subject(key))),
        DiagnosticObject::StoreState => write!(output, "{}.state", Hex(namespace.bytes())),
        DiagnosticObject::Journal(key) => write!(output, "{}.journal", Hex(&operation(key))),
        DiagnosticObject::Payload(key, revision, kind) => {
            let kind = match kind {
                AttachmentKind::Startup => 1,
                AttachmentKind::Fault => 2,
                AttachmentKind::Runtime => 3,
                AttachmentKind::Log => 4,
                AttachmentKind::SymbolizedFault => 5,
                AttachmentKind::Minidump => 6,
            };
            write!(output, "{}.p-{:016x}-{kind}.payload", Hex(&subject(key)), revision.get())
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
    fn leaf(object: DiagnosticObject, namespace: NamespaceId) -> Result<String, std::fmt::Error> {
        let mut output = String::new();
        write_leaf(object, namespace, &mut output)?;
        Ok(output)
    }
    #[test]
    fn diagnostic_generation_typed_groups_keep_original_leaf_and_subject_binding()
    -> Result<(), Box<dyn std::error::Error>> {
        let namespace = NamespaceId::new([1; 32]);
        let key = IncidentKey::new(IncidentSubject::Supervisor(CoordinatorKey {
            namespace,
            epoch: CoordinatorEpoch::new([2; 16])?,
        }));
        let revision = IncidentRevision::new(7)?;
        let log = DiagnosticObject::Payload(key, revision, AttachmentKind::Log);
        let fault = DiagnosticObject::Payload(key, revision, AttachmentKind::Fault);
        assert_eq!(object_payload_group(log), object_payload_group(fault));
        assert_ne!(
            object_payload_group(log),
            object_payload_group(DiagnosticObject::Payload(
                key,
                IncidentRevision::new(8)?,
                AttachmentKind::Log
            ))
        );
        assert_eq!(binding(log, namespace), (DiagnosticEntryClass::Payload, subject(key)));
        assert_eq!(
            leaf(log, namespace)?,
            format!(
                "{}.p-0000000000000007-4.payload",
                crate::durable_storage::protected::record_file::hex(&subject(key))
            )
        );
        assert_eq!(object_payload_group(DiagnosticObject::Summary(key)), None);
        assert_eq!(object_payload_group(DiagnosticObject::Seed(key)), None);
        assert_eq!(object_payload_group(DiagnosticObject::StoreState), None);
        Ok(())
    }
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
            leaf(DiagnosticObject::Journal(first), namespace)?,
            leaf(DiagnosticObject::Journal(later), namespace)?
        );
        assert_ne!(
            leaf(
                DiagnosticObject::Seed(IncidentKey::new(IncidentSubject::Supervisor(
                    first.owner()
                ))),
                namespace
            )?,
            leaf(
                DiagnosticObject::Seed(IncidentKey::new(IncidentSubject::Supervisor(
                    later.owner()
                ))),
                namespace
            )?
        );
        Ok(())
    }
}
