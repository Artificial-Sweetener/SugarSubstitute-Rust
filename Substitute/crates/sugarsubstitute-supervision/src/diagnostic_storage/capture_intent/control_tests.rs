// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify per-effect liveness without repeating complete pinned-file observations.
use super::*;

struct Effects<'a> {
    fixture: &'a DiagnosticStorageFixture,
    generation: &'a ProtectedCaptureReadLease,
    calls: usize,
    observations: usize,
}
// SAFETY: only the external native effect is faked. The actual admitted generation,
// storage, immutable read pins and original controls mint and retain every fence.
#[allow(unsafe_code)]
unsafe impl DiagnosticCaptureNativeSource for Effects<'_> {
    type Error = SourceFailure;
    fn settle(&mut self, fence: &mut DiagnosticCaptureFence<'_>) -> Result<(), Self::Error> {
        self.calls += 1;
        assert!(fence.matches_generation(self.generation));
        let before = self.fixture.file_observations();
        for _ in 0..64 {
            assert!(fence.check_pinned_current().is_ok());
        }
        assert_eq!(self.fixture.file_observations(), before);
        assert!(fence.check_current().is_ok());
        assert_eq!(self.fixture.file_observations(), before + 3);
        self.observations = before + 3;
        Ok(())
    }
}

#[test]
fn capture_effect_controls_keep_actual_pins_without_rehashing_and_full_post_observation_remains()
-> Result<(), Box<dyn std::error::Error>> {
    actual(|context, storage, operation, fixture, generation| {
        let (mut journal, mut summary, mut payload) = reads(storage, operation, fixture, context)?;
        let mut intent = storage.adopt_capture_intent(
            &mut journal,
            &mut summary,
            Some(&mut payload),
            operation,
            context,
        )?;
        let mut source = Effects { fixture, generation, calls: 0, observations: 0 };
        storage.settle_capture(&mut intent, &mut source, operation, context)?;
        assert_eq!(source.calls, 1);
        assert!(fixture.file_observations() >= source.observations + 3);
        summary.retire(context)?;
        payload.retire(context)?;
        journal.retire(context)?;
        Ok(())
    })
}
