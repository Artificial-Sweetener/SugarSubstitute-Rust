// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise raw actual-result path decoding without native filesystem effects.
use super::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn borrowed_ancestors_keep_root_first_order_and_original_path_storage() -> TestResult {
    for value in [r"E:\alpha\beta\leaf", r"\\?\e:\alpha\beta\leaf"] {
        let path = Path::new(value);
        let ancestors = ancestor_paths(path)?.collect::<Vec<_>>();
        let parent = path.parent().ok_or("test parent absent")?;
        assert_eq!(ancestors.len(), 3);
        assert_eq!(ancestors[2], parent);
        assert!(matches!(
            ancestors[0].components().next_back(),
            Some(std::path::Component::RootDir)
        ));
        for ancestor in ancestors {
            assert_eq!(
                ancestor.as_os_str().as_encoded_bytes().as_ptr(),
                path.as_os_str().as_encoded_bytes().as_ptr()
            );
            assert!(ancestor.has_root());
            assert!(!matches!(
                ancestor.components().next_back(),
                Some(std::path::Component::Prefix(_))
            ));
        }
    }
    Ok(())
}
#[test]
fn borrowed_ancestors_preserve_exact_parent_component_limit() -> TestResult {
    let mut parent = PathBuf::from(r"\\?\E:\");
    for _ in 0..126 {
        parent.push("a");
    }
    assert_eq!(parent.components().count(), 128);
    let accepted = parent.join("leaf");
    assert_eq!(ancestor_paths(&accepted)?.count(), 127);
    parent.push("a");
    let rejected = parent.join("leaf");
    let error = ancestor_paths(&rejected).err().ok_or("component bound rejection absent")?;
    assert_eq!(error, "path_component_bound");
    Ok(())
}
#[test]
fn actual_native_path_decoding_preserves_isolated_high_and_low_surrogates() -> TestResult {
    let units = [92, 92, 63, 92, 69, 58, 92, 0xd800, 97, 0xdc01];
    let path = decode_native_result(&units)?;
    assert!(path.to_str().is_none());
    assert_eq!(path.as_os_str().encode_wide().collect::<Vec<_>>(), units);
    validate_canonical_local(&path)?;
    Ok(())
}
#[test]
fn actual_native_path_decoding_keeps_exact_case_and_accepts_drive_root() -> TestResult {
    for value in [r"\\?\E:\", r"\\?\e:\MixedCase\漢字"] {
        let units = value.encode_utf16().collect::<Vec<_>>();
        let path = decode_native_result(&units)?;
        assert_eq!(path.as_os_str().encode_wide().collect::<Vec<_>>(), units);
    }
    Ok(())
}
#[test]
fn actual_native_path_decoding_rejects_unc_device_relative_and_nul() {
    for value in [
        r"E:\regular",
        r"\\?\UNC\server\share",
        r"\\.\pipe\endpoint",
        r"\\?\E:relative",
        "\\\\?\\E:\\nul\0tail",
    ] {
        assert!(decode_native_result(&value.encode_utf16().collect::<Vec<_>>()).is_err());
    }
    assert!(decode_native_result(&[]).is_err());
}
#[test]
fn actual_native_path_decoding_enforces_exact_existing_unit_bound() -> TestResult {
    let mut units = vec![92, 92, 63, 92, 69, 58, 92];
    units.resize(32766, 97);
    assert_eq!(decode_native_result(&units)?.as_os_str().encode_wide().count(), 32766);
    units.push(97);
    assert!(decode_native_result(&units).is_err());
    Ok(())
}

#[test]
fn bounded_shell_scan_preserves_raw_local_path_units_and_rejects_missing_terminator() -> TestResult
{
    let mut units = vec![69, 58, 92, 0xd800, 97, 0xdc01, 0];
    // SAFETY: This owned buffer includes the terminator inside its exact advertised bound.
    let path = unsafe { read_native_path(PWSTR(units.as_mut_ptr()), units.len()) }?;
    assert!(path.to_str().is_none());
    assert_eq!(path.as_os_str().encode_wide().collect::<Vec<_>>(), units[..units.len() - 1]);
    // SAFETY: The scan cannot read beyond the valid owned allocation and rejects this shorter bound.
    assert!(unsafe { read_native_path(PWSTR(units.as_mut_ptr()), units.len() - 1) }.is_err());
    // SAFETY: The null pointer rejects before any dereference.
    assert!(unsafe { read_native_path(PWSTR::null(), units.len()) }.is_err());
    Ok(())
}

#[test]
fn path_phase_parity_preserves_local_case_roots_and_raw_surrogates() -> TestResult {
    for mut units in [
        vec![69, 58, 92],
        vec![92, 92, 63, 92, 101, 58, 92],
        vec![69, 58, 92, 0xd800, 97, 0xdc01],
        vec![92, 92, 63, 92, 69, 58, 92, 0xdc01, 97, 0xd800],
    ] {
        let path = PathBuf::from(OsString::from_wide(&units));
        let encoded = wide(&path)?;
        units.push(0);
        assert_eq!(encoded, units);
        // SAFETY: The owned exact NUL buffer stays live throughout the bounded scan.
        let scanned = unsafe { read_native_path(PWSTR(units.as_mut_ptr()), units.len()) }?;
        assert_eq!(scanned.as_os_str(), path.as_os_str());
    }
    Ok(())
}

#[test]
fn path_phase_parity_keeps_original_rejection_operations() -> TestResult {
    for (value, expected) in [
        ("", "non_local_path"),
        ("relative", "non_local_path"),
        (r"E:relative", "non_local_path"),
        (r"\\server\share", "non_local_path"),
        (r"\\?\UNC\server\share", "non_local_path"),
        (r"\\.\pipe\endpoint", "non_local_path"),
        ("E:\\nul\0tail", "path_shape"),
        ("\\\\?\\E:\\nul\0tail", "path_shape"),
    ] {
        let path = Path::new(value);
        let wide_error = wide(path).err().ok_or("original wide rejection absent")?;
        let canonical_error =
            validate_canonical_local(path).err().ok_or("original canonical rejection absent")?;
        assert_eq!(wide_error.operation(), expected);
        assert_eq!(canonical_error, expected);
    }
    for value in [r"relative", r"E:relative", r"\\?\UNC\server\share"] {
        let mut units = value.encode_utf16().chain(std::iter::once(0)).collect::<Vec<_>>();
        // SAFETY: The owned exact NUL buffer stays live throughout the bounded scan.
        let error = unsafe { read_native_path(PWSTR(units.as_mut_ptr()), units.len()) }
            .err()
            .ok_or("original bounded Shell rejection absent")?;
        assert_eq!(error.operation(), "non_local_path");
    }
    Ok(())
}

#[test]
fn path_phase_parity_keeps_exact_native_unit_limit_with_raw_surrogate() -> TestResult {
    let mut units = vec![92, 92, 63, 92, 69, 58, 92];
    units.resize(32766, 97);
    units[32765] = 0xd800;
    let path = PathBuf::from(OsString::from_wide(&units));
    let encoded = wide(&path)?;
    assert_eq!(encoded.len(), 32767);
    assert_eq!(&encoded[..32766], units);
    assert_eq!(encoded[32766], 0);
    assert_eq!(decode_native_result(&units)?.as_os_str(), path.as_os_str());
    units.push(97);
    let oversized = PathBuf::from(OsString::from_wide(&units));
    let error = wide(&oversized).err().ok_or("original unit bound rejection absent")?;
    assert_eq!(error.operation(), "path_shape");
    let error = validate_canonical_local(&oversized)
        .err()
        .ok_or("original canonical unit bound rejection absent")?;
    assert_eq!(error, "path_shape");
    Ok(())
}
