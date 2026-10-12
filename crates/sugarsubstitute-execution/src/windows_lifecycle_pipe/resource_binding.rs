// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Bind each physical output owner to its original ledger and actual native child.
use super::*;
impl Lease {
    pub(super) fn bind_original_context(&mut self) {
        if let Some(symbol) = &self.symbol {
            self.bind_resources(crate::native_resources::EffectContext::new(symbol));
        }
    }
    /// Retain the actual native child and original main before native preparation.
    pub(super) fn bind_effect_context(&mut self, context: &JobContext) {
        if let Some(symbol) = &self.symbol {
            self.bind_resources(
                crate::native_resources::EffectContext::new(symbol).for_child(context),
            );
        }
    }
    fn bind_resources(&mut self, original: crate::native_resources::EffectContext) {
        if let Some(symbol) = &self.symbol {
            for handle in [
                &mut self.input,
                &mut self.pipe,
                &mut self.own_token,
                &mut self.peer_token,
                &mut self.observed_token,
                &mut self.message_token,
                &mut self.observed_process,
            ] {
                handle.attach(symbol.handles.clone(), original.clone());
            }
            self.section.attach(symbol.handles.clone(), original);
        }
    }
}
