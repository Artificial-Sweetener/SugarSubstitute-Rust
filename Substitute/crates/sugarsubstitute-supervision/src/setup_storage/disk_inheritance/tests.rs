// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own native disk-handle duplication and unique initial-entry adoption for installation processes.
use super::*;

#[test]
fn gui_standard_stream_absence_is_allowed_but_native_query_failure_is_preserved() -> io::Result<()>
{
    use windows::core::{Error, HRESULT};
    assert_eq!(optional_standard(Err(Error::from_hresult(HRESULT(0))))?, None);
    assert!(optional_standard(Err(Error::from_hresult(HRESULT(0x80070006_u32 as i32)))).is_err());
    Ok(())
}
