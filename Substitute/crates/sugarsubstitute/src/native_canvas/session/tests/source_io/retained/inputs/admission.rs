// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Answer input work that execution can never admit instead of leaving its request waiting.
use super::*;

/// A source whose acquisition needs more working memory than any executor provides.
struct OversizedSource;

impl SourceAcquirer for OversizedSource {
    fn reservation_bytes(&self) -> u64 {
        1 << 62
    }

    fn acquire(
        &self,
        _: RuntimeCache,
        _: &MemoryLease,
        _: &JobContext,
    ) -> io::Result<SelectedOriginals> {
        Err(io::Error::other("An inadmissible acquisition must never run"))
    }

    fn cancel(&self) {}

    fn retire(&self, _: &JobContext) -> io::Result<()> {
        Ok(())
    }

    fn settled(&self) -> bool {
        true
    }
}

#[test]
fn input_work_execution_can_never_admit_is_answered_with_a_typed_failure()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    let base =
        ComfyFile { reference: Arc::from("input/huge.png"), acquirer: Arc::new(OversizedSource) };
    let reply =
        fixture.sync(vec![surface("image", SurfaceBase::File(base), vec![mask("only", None)?])])?;
    let failure = reply
        .surfaces
        .iter()
        .find_map(|outcome| match outcome {
            SurfaceOutcome::Failed { key, failure } if &**key == "image" => Some(failure.clone()),
            _ => None,
        })
        .ok_or_else(|| {
            format!("The inadmissible surface was not answered as failed: {:?}", reply.surfaces)
        })?;
    assert!(failure.to_string().contains("more memory than the canvas can provide"), "{failure}");
    Ok(())
}
