// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Input admission outlives mutable navigation and is returned only with snapshot disposal.
use super::*;
use sugarsubstitute_contracts::{configuration::Theme, execution::MemoryRelease};

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

#[test]
fn snapshot_keeps_original_display_state_until_its_claim_is_released() -> io::Result<()> {
    let mut flow = SetupFlow::new("E:\\初期".into()).map_err(io::Error::other)?;
    let view = View {
        surface: PaintSpec { width: 720, height: 560, scale: 1.0, theme: Theme::Dark },
        scroll: 0.0,
        now: Duration::ZERO,
        interaction: Interaction::default(),
        text_revisions: [0; 5],
        accent: sugarsubstitute_ui::accent::AccentPalette::PRODUCT,
    };
    let lease = MemoryLease::new(SNAPSHOT_BYTES, Box::new(Release));
    let old = Snapshot::capture(&flow, view, &lease)?;
    let request = flow.set_locale("ja", Duration::ZERO).map_err(io::Error::other)?;
    flow.receive(
        request.id,
        sugarsubstitute_application::setup_flow::Outcome::Completed(
            sugarsubstitute_application::setup_flow::Completed::Locale,
        ),
        Duration::ZERO,
    )
    .map_err(io::Error::other)?;
    assert_eq!(old.selected_locale, "en");
    assert_eq!(old.locale, "en");
    assert_eq!(old.paths.installation, "E:\\初期");
    assert!(Snapshot::capture(&flow, view, &lease).is_err());
    drop(old);
    let new = Snapshot::capture(&flow, view, &lease)?;
    assert_eq!(new.locale, "ja");
    assert_eq!(new.selected_locale, "ja");
    Ok(())
}
