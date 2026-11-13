// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Format canonical diagnostic names in bounded stack storage before path admission.
use super::*;
use std::fmt::{self, Write};

pub(super) const LEAF_BYTES: usize = 197;

/// Keep every canonical leaf byte inline, including complete stage identities.
pub(crate) struct Leaf {
    bytes: [u8; LEAF_BYTES],
    length: usize,
}
impl Leaf {
    pub(super) const fn empty() -> Self {
        Self { bytes: [0; LEAF_BYTES], length: 0 }
    }
    pub(crate) fn object(
        object: DiagnosticObject,
        namespace: NamespaceId,
    ) -> Result<Self, fmt::Error> {
        let mut leaf = Self::empty();
        layout::write_leaf(object, namespace, &mut leaf)?;
        Ok(leaf)
    }
    pub(super) fn inventory(
        observed: &diagnostic::inventory::ObservedEntry,
    ) -> Result<Self, fmt::Error> {
        let mut leaf = Self::empty();
        write!(leaf, "{}.{}", Hex(&observed.digest), observed.class.suffix())?;
        Ok(leaf)
    }
    pub(super) fn stage(
        object: DiagnosticObject,
        namespace: NamespaceId,
        operation: LogicalStoreOperationKey,
        nonce: &[u8],
    ) -> Result<Self, fmt::Error> {
        let mut leaf = Self::object(object, namespace)?;
        write!(leaf, ".{}.{}.stage", Hex(&layout::operation(operation)), Hex(nonce))?;
        Ok(leaf)
    }
    pub(super) fn recovery_prefix(
        object: DiagnosticObject,
        namespace: NamespaceId,
        operation: LogicalStoreOperationKey,
    ) -> Result<Self, fmt::Error> {
        let mut leaf = Self::object(object, namespace)?;
        write!(leaf, ".{}.", Hex(&layout::operation(operation)))?;
        Ok(leaf)
    }
    pub(crate) fn text(&self) -> Result<&str, fmt::Error> {
        std::str::from_utf8(&self.bytes[..self.length]).map_err(|_| fmt::Error)
    }
}
impl Write for Leaf {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self
            .length
            .checked_add(text.len())
            .filter(|end| *end <= LEAF_BYTES)
            .ok_or(fmt::Error)?;
        self.bytes[self.length..end].copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}
pub(super) struct Hex<'a>(pub(super) &'a [u8]);
impl fmt::Display for Hex<'_> {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(output, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_contracts::{
        incident::{
            AttachmentKind, IncidentKey, IncidentOperationId, IncidentRevision, IncidentSubject,
        },
        lifecycle::{CoordinatorEpoch, CoordinatorKey},
    };

    #[test]
    fn longest_canonical_stage_uses_exact_inline_bound_and_original_bytes()
    -> Result<(), Box<dyn std::error::Error>> {
        let namespace = NamespaceId::new([1; 32]);
        let owner = CoordinatorKey { namespace, epoch: CoordinatorEpoch::new([2; 16])? };
        let incident = IncidentKey::new(IncidentSubject::Supervisor(owner));
        let object = DiagnosticObject::Payload(
            incident,
            IncidentRevision::new(u64::MAX)?,
            AttachmentKind::Minidump,
        );
        let operation = LogicalStoreOperationKey::new(owner, IncidentOperationId::new(u64::MAX)?);
        let nonce = [3; 16];
        let leaf = Leaf::stage(object, namespace, operation, &nonce)?;
        let mut original = String::new();
        layout::write_leaf(object, namespace, &mut original)?;
        write!(original, ".{}.{}.stage", Hex(&layout::operation(operation)), Hex(&nonce))?;
        assert_eq!(leaf.text()?, original);
        assert_eq!(leaf.text()?.len(), LEAF_BYTES);
        Ok(())
    }

    #[test]
    fn rejected_stack_append_preserves_every_predecessor_byte()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut leaf = Leaf::empty();
        leaf.write_str("previous")?;
        let excess = [b'x'; LEAF_BYTES];
        assert!(leaf.write_str(std::str::from_utf8(&excess)?).is_err());
        assert_eq!(leaf.text()?, "previous");
        Ok(())
    }
}
