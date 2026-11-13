// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain temporary originals once by content and decide which no document needs.
use super::*;
use crate::input_documents::bases::RetainedBases;

impl Fixture {
    fn bases(&self) -> RetainedBases {
        RetainedBases::new(self.store.directory())
    }

    /// Run `work` as one admitted job and return its result.
    fn job<T: Send + 'static>(
        &self,
        work: impl FnOnce(&JobContext) -> Result<T, StoreError> + Send + 'static,
    ) -> TestResult<T> {
        let mut job = self.client.submit_blocking(
            &self.scope,
            self.spec(8 * 1024 * 1024)?,
            move |context| work(&context),
        )?;
        let completion = self.settle(|| Ok(job.try_take_completion()))?;
        match completion.outcome {
            JobOutcome::Success(value) => Ok(value),
            JobOutcome::Failure(cause) => Err(cause.into()),
            other => Err(format!("Job stopped: {:?}", other.kind()).into()),
        }
    }

    fn retain(&self, reference: &'static str, bytes: &'static [u8]) -> TestResult {
        let (bases, memory) = (self.bases(), self.memory.clone());
        self.job(move |context| bases.retain(reference, bytes, &memory, context))
    }

    fn find(&self, reference: &'static str) -> TestResult<Option<Vec<u8>>> {
        let (bases, memory) = (self.bases(), self.memory.clone());
        self.job(move |context| {
            Ok(bases.find(reference, 1024, &memory, context)?.map(|(bytes, _)| bytes))
        })
    }

    fn unreferenced(&self, keep: &'static [&'static str]) -> TestResult<Vec<String>> {
        let bases = self.bases();
        self.job(move |context| bases.unreferenced(keep.iter().copied(), context))
    }

    fn leaves(&self) -> TestResult<Vec<String>> {
        let mut leaves: Vec<String> = std::fs::read_dir(self.store.directory().join("bases"))?
            .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
            .collect::<Result<_, _>>()?;
        leaves.sort();
        Ok(leaves)
    }
}

/// The lowercase SHA-256 spelling naming an original.
fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

const FIRST: &str = "/view?filename=a.png&subfolder=&type=temp";
const SECOND: &str = "/view?filename=b.png&subfolder=&type=temp";
const THIRD: &str = "/view?filename=c.png&subfolder=&type=output";

#[test]
fn equal_originals_are_stored_once_and_found_by_every_reference() -> TestResult {
    let fixture = Fixture::new()?;
    assert_eq!(fixture.unreferenced(&[])?, Vec::<String>::new(), "no bases folder yet");
    fixture.retain(FIRST, b"same original")?;
    fixture.retain(SECOND, b"same original")?;
    let leaves = fixture.leaves()?;
    assert_eq!(leaves.iter().filter(|leaf| leaf.ends_with(".image")).count(), 1);
    assert_eq!(leaves.iter().filter(|leaf| leaf.ends_with(".reference")).count(), 2);
    assert_eq!(fixture.find(FIRST)?.as_deref(), Some(&b"same original"[..]));
    assert_eq!(fixture.find(SECOND)?.as_deref(), Some(&b"same original"[..]));
    assert_eq!(fixture.find(THIRD)?, None);
    Ok(())
}

#[test]
fn only_originals_no_kept_reference_names_are_selected_for_retirement() -> TestResult {
    let fixture = Fixture::new()?;
    fixture.retain(FIRST, b"shared")?;
    fixture.retain(SECOND, b"shared")?;
    fixture.retain(THIRD, b"alone")?;
    std::fs::write(fixture.store.directory().join("bases").join("notes.txt"), b"not ours")?;
    let before = fixture.leaves()?;
    // Keeping one of two references to a shared original keeps that original.
    let selected = fixture.unreferenced(&[FIRST])?;
    let image = |bytes: &[u8]| format!("{}.image", digest(bytes));
    assert_eq!(selected.len(), 3, "{selected:?}");
    assert!(selected.contains(&image(b"alone")));
    assert!(!selected.contains(&image(b"shared")));
    assert!(!selected.iter().any(|leaf| leaf == "notes.txt"), "unknown files are never selected");
    assert_eq!(selected.iter().filter(|leaf| leaf.ends_with(".reference")).count(), 2);
    // Nothing kept selects every original and reference, and deciding removes nothing.
    assert_eq!(fixture.unreferenced(&[])?.len(), 5);
    assert_eq!(fixture.leaves()?, before);
    Ok(())
}

#[test]
fn a_retained_original_that_no_longer_matches_its_digest_is_never_used() -> TestResult {
    let fixture = Fixture::new()?;
    fixture.retain(FIRST, b"authored on")?;
    let image =
        fixture.store.directory().join("bases").join(format!("{}.image", digest(b"authored on")));
    std::fs::write(&image, b"changed")?;
    assert_eq!(fixture.find(FIRST)?, None);
    assert_eq!(std::fs::read(&image)?, b"changed", "the changed file stays as evidence");
    Ok(())
}
