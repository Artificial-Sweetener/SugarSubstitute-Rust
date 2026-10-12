// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain original read admission before a stage exists and until actual closure.
use super::*;
use super::{
    release::{generation, ordinary},
    role_fixture::with_record,
};
#[test]
fn fixed_record_failed_predecessor_close_retains_binding_until_checked_retirement() -> TestResult {
    let mut old = generation(resolved().namespace, 1)?;
    old.state = crate::native_claims::RecordState::Released;
    let original = old.encode();
    let next = generation(resolved().namespace, 2)?;
    with_record::<128>(Some(original.to_vec()), move |file, fake, context| {
        fake.fail_close(true);
        assert!(read_with_boundary(file, context, fake).is_err());
        assert_eq!(fake.live(), 2);
        assert!(lock(&file.stage).is_none());
        assert!(lock(&file.publication).is_some());
        fake.fail_close(false);
        assert!(
            matches!(read_with_boundary(file,context,fake)?,RecordRead::Present(bytes) if bytes==original)
        );
        assert_eq!(fake.live(), 2);
        assert!(lock(&file.publication).is_some());
        assert!(ordinary(file, &next, fake, context).is_err());
        assert_eq!(fake.effect_count(), 0);
        let actual = retirement::retire_with_boundary(file, None, context, fake)?;
        assert!(
            matches!(actual,PublicationRetirement::Preserved(RecordRead::Present(bytes)) if bytes==original)
        );
        assert_eq!(fake.live(), 1);
        assert!(lock(&file.publication).is_none());
        Ok(())
    })
}
