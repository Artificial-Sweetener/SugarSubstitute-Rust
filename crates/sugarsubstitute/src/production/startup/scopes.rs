// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep original child and render scope identities distinct before worker creation.
use std::error::Error;
use sugarsubstitute_contracts::execution::ScopeSpec;

/// Original scope identities are assigned by process composition, never by rendering.
pub(crate) struct StartupScopes {
    /// Exclusive render lifetime with its original supplied deadline.
    pub(crate) render: ScopeSpec,
    /// Genuine child admission lifetime with its original supplied deadline.
    pub(crate) child: ScopeSpec,
}
impl StartupScopes {
    /// Reject aliasing before creating execution or performing any native work.
    pub(super) fn validate(&self) -> Result<(), Box<dyn Error>> {
        if self.render.id == self.child.id {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Child and render scope identities alias",
            )
            .into());
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_contracts::execution::{Moment, ScopeId};
    #[test]
    fn scope_alias_rejected_without_rewriting_original_deadlines() -> Result<(), Box<dyn Error>> {
        let render = ScopeSpec { id: ScopeId::new(1)?, deadline: Some(Moment::from_nanos(100)) };
        let mut scopes = StartupScopes { render, child: render };
        assert!(scopes.validate().is_err());
        assert_eq!(scopes.render, render);
        scopes.child.id = ScopeId::new(2)?;
        scopes.child.deadline = Some(Moment::from_nanos(50));
        scopes.validate()?;
        assert_eq!(scopes.render, render);
        assert_eq!(scopes.child.deadline, Some(Moment::from_nanos(50)));
        Ok(())
    }
}
