// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Test helper/session exhaustion and complete persisted operation identity.
use super::*;

#[test]
fn incident_helper_numeric_ids_reject_zero_and_never_wrap() -> Result<(), IncidentError> {
    assert_eq!(DiagnosticSessionId::new(0), Err(IncidentError::InvalidIdentity));
    assert_eq!(IncidentOperationId::new(0), Err(IncidentError::InvalidIdentity));
    let first_session = DiagnosticSessionId::new(1)?;
    let first_operation = IncidentOperationId::new(1)?;
    assert_eq!(first_session.next()?.get(), 2);
    assert_eq!(first_operation.next()?.get(), 2);
    assert_eq!(first_session.get(), 1);
    assert_eq!(first_operation.get(), 1);
    let last_session = DiagnosticSessionId::new(u64::MAX)?;
    let last_operation = IncidentOperationId::new(u64::MAX)?;
    assert_eq!(last_session.next(), Err(IncidentError::OperationExhausted));
    assert_eq!(last_operation.next(), Err(IncidentError::OperationExhausted));
    assert_eq!(last_session.get(), u64::MAX);
    assert_eq!(last_operation.get(), u64::MAX);
    Ok(())
}

#[test]
fn incident_helper_sessions_preserve_owner_role_and_session()
-> Result<(), Box<dyn std::error::Error>> {
    let owner = owner()?;
    let session = DiagnosticSessionId::new(1)?;
    let key = DiagnosticKey::new(owner, DiagnosticRole::Storage, session);
    assert_eq!(key.owner(), owner);
    assert_eq!(key.role(), DiagnosticRole::Storage);
    assert_eq!(key.session(), session);
    let owners = [
        CoordinatorKey { namespace: NamespaceId::new([7; 32]), ..owner },
        CoordinatorKey { epoch: CoordinatorEpoch::new([8; 16])?, ..owner },
    ];
    for changed in owners {
        assert_ne!(key, DiagnosticKey::new(changed, key.role(), session));
    }
    for role in [DiagnosticRole::Symbolize, DiagnosticRole::Reporter] {
        assert_ne!(key, DiagnosticKey::new(owner, role, session));
    }
    assert_ne!(key, DiagnosticKey::new(owner, key.role(), session.next()?));
    Ok(())
}

#[test]
fn incident_helper_historical_operation_one_is_distinct_from_new_owner_operation_one()
-> Result<(), Box<dyn std::error::Error>> {
    let owner = owner()?;
    let id = IncidentOperationId::new(1)?;
    let original = LogicalStoreOperationKey::new(owner, id);
    let recovery_owner = CoordinatorKey { epoch: CoordinatorEpoch::new([9; 16])?, ..owner };
    let recovery = LogicalStoreOperationKey::new(recovery_owner, id);
    assert_ne!(original, recovery);
    assert_eq!(original.owner(), owner);
    assert_eq!(original.id(), id);
    assert_eq!(recovery.owner(), recovery_owner);
    assert_eq!(recovery.id(), id);
    let other_namespace = CoordinatorKey { namespace: NamespaceId::new([8; 32]), ..owner };
    assert_ne!(original, LogicalStoreOperationKey::new(other_namespace, id));
    assert_ne!(original, LogicalStoreOperationKey::new(owner, id.next()?));
    Ok(())
}
