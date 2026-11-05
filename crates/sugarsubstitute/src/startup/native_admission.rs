// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit the application canvas and composition owners before the native lifetime starts.
//!
//! Each owner receives its own scope, ordered after the child and render scopes, and only
//! while startup is inert; admission after start or a reused scope is rejected.

use super::*;

impl<B: StartupBackend> StartupHost<B> {
    /// Retain one empty application canvas before the native startup lifetime begins.
    /// `anchor` is the installation root supplied by the parent; the runtime cache owner opens
    /// its schema-qualified scope below it and validates it on a worker.
    #[cfg(any(feature = "windows-native-host", test))]
    pub(crate) fn configure_canvas(
        &mut self,
        anchor: std::path::PathBuf,
        health: crate::diagnostics::DiagnosticHealth,
        scope: sugarsubstitute_contracts::execution::ScopeSpec,
    ) -> Result<(), Box<dyn Error>> {
        if self.started
            || self.canvas.is_some()
            || scope.deadline.is_some()
            || scope.id <= self.scopes.child.id
            || scope.id <= self.scopes.render.id
        {
            return Err("Application canvas has an invalid or already active scope".into());
        }
        #[cfg(feature = "native-composition")]
        if self.composition.as_ref().is_some_and(|composition| scope.id <= composition.scope_id()) {
            return Err("Application canvas scope precedes composition execution".into());
        }
        if !anchor.is_absolute() {
            return Err("Application canvas cache anchor must be absolute".into());
        }
        let maximum = crate::native_canvas::budget::largest_canvas_extent()?;
        let (governor, retained) = memory::MemoryGovernor::open()?;
        // The renderer policy is chosen when the canvas starts, so fund a GPU session.
        self.execution.reserve_canvas_capacity(crate::native_canvas::budget::reservation_bytes(
            maximum, true, retained,
        ))?;
        self.canvas = Some(canvas::EmbeddedCanvas::new(
            anchor,
            health,
            self.clock.clone(),
            self.jobs.clone(),
            scope,
            canvas::CanvasFunding { maximum_extent: maximum, retained_document_bytes: retained },
        ));
        let _first = self.memory.replace(governor);
        Ok(())
    }

    /// Reserve the composition-wait scope before native window construction begins.
    #[cfg(feature = "native-composition")]
    pub(crate) fn configure_composition(
        &mut self,
        scope: sugarsubstitute_contracts::execution::ScopeSpec,
    ) -> Result<(), Box<dyn Error>> {
        if self.started
            || self.composition.is_some()
            || self.canvas.is_some()
            || scope.deadline.is_some()
            || scope.id <= self.scopes.child.id
            || scope.id <= self.scopes.render.id
        {
            return Err("Composition wait scope is invalid or already active".into());
        }
        self.composition = Some(composition::CompositionWork::new(scope, self.jobs.clone()));
        Ok(())
    }
}
