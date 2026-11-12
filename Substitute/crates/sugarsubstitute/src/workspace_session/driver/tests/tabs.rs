// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Finish a restore on the tabs Comfy reopened, however they differ from the checkpoint.
use super::*;

/// Comfy owns workflow tabs: it may reopen fewer tabs than the checkpoint names and tabs the
/// checkpoint never saw. The restore binds the entries Comfy reopened and finishes beside the rest.
#[test]
fn a_restore_finishes_on_the_tabs_comfy_reopened_beside_tabs_it_never_saved() -> TestResult {
    let (mut driver, mut fence) = fixture()?;
    let (request, transfer) = encoding(ready(&mut driver, fence.clone())?)?;
    driver.encoding_stopped(&request, &transfer)?;
    let checkpoint = driver
        .restored
        .as_ref()
        .and_then(|value| value.checkpoint.as_ref())
        .map(|(_, checkpoint)| checkpoint.clone())
        .ok_or("Missing saved checkpoint")?;
    assert!(checkpoint.entries.len() > 1, "the fixture must name more than one entry");
    driver.restore.as_mut().ok_or("Restore disappeared")?.sent = true;
    fence.revision += 1;
    for binding in &mut fence.bindings {
        binding.runtime_session = format!("workflow:{}", binding.entry_id);
        binding.generation = 0;
    }
    // Comfy did not reopen the last saved tab, and reopened one the checkpoint never saw.
    let dropped = checkpoint.entries.last().ok_or("Missing entry")?.id.clone();
    fence.bindings.retain(|binding| binding.entry_id != dropped);
    let mut unsaved = fence.bindings.first().ok_or("Missing binding")?.clone();
    unsaved.entry_id = "00000000-0000-4000-8000-0000000000aa".into();
    unsaved.runtime_session = "workflow:comfy-only".into();
    fence.bindings.push(unsaved.clone());
    ready_state(&mut driver, fence.clone(), true)?;
    let restored: Vec<_> = fence
        .bindings
        .iter()
        .filter(|binding| checkpoint.entries.iter().any(|entry| entry.id == binding.entry_id))
        .cloned()
        .collect();
    assert_eq!(restored.len(), checkpoint.entries.len() - 1);
    while driver.next_message().is_some() {}
    let acknowledgement = driver.message(
        request,
        SessionOperation::Acknowledged {
            checkpoint_id: checkpoint.id.clone(),
            revision: checkpoint.revision,
            phase: "materialized".into(),
            bindings: restored.clone(),
        },
    );
    driver.receive(acknowledgement, Moment::from_nanos(1))?;
    assert!(driver.restore.is_none(), "the restore must finish beside the unsaved tab");
    let finished = std::iter::from_fn(|| driver.next_message())
        .find_map(|message| match message.operation {
            SessionOperation::Acknowledged { phase, bindings, .. } if phase == "restored" => {
                Some(bindings)
            }
            _ => None,
        })
        .ok_or("Missing restored acknowledgement")?;
    assert_eq!(finished, restored);
    Ok(())
}
