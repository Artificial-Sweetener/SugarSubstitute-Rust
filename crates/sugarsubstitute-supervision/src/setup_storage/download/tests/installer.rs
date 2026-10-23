// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Installer envelopes supply exactly the repository-authenticated bare package interval.
use super::*;

fn archive() -> Vec<u8> {
    crate::setup_storage::test_package::seal(b"{}", b"inert compressed fixture")
}

fn signed(bytes: &[u8]) -> Result<Target, Box<dyn std::error::Error + Send + Sync>> {
    let key = key(63)?;
    let mut trust = RepositoryTrust::bootstrap(&root(&key, 1, None)?)?;
    trust.refresh(Set::new(&key, [1; 3], bytes, EXPIRES)?.metadata(), NOW)?;
    Ok(trust.target(NAME, NOW)?)
}

/// Structural PE fixture; these certificates convey no authority and are never executed.
fn envelope(archive: &[u8], certificate: Option<bool>) -> Vec<u8> {
    let mut bytes = vec![0; 512];
    bytes.extend_from_slice(archive);
    if let Some(pe32) = certificate {
        bytes[..2].copy_from_slice(b"MZ");
        bytes[60..64].copy_from_slice(&128_u32.to_le_bytes());
        bytes[128..132].copy_from_slice(b"PE\0\0");
        bytes[148..150].copy_from_slice(&240_u16.to_le_bytes());
        bytes[152..154].copy_from_slice(&(if pe32 { 0x10b_u16 } else { 0x20b }).to_le_bytes());
        let (count, entry) = if pe32 { (244, 280) } else { (260, 296) };
        bytes[count..count + 4].copy_from_slice(&16_u32.to_le_bytes());
        let start = (bytes.len() + 7) & !7;
        bytes[entry..entry + 4].copy_from_slice(&(start as u32).to_le_bytes());
        bytes[entry + 4..entry + 8].copy_from_slice(&16_u32.to_le_bytes());
        bytes.resize(start, 0);
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&[0, 2, 2, 0, 1, 2, 3, 4, 0, 0, 0, 0]);
    }
    bytes
}

#[test]
fn installer_import_resumes_bare_archive_and_excludes_executable_and_certificates() -> TestResult {
    worker::run_reserved(DOWNLOAD_RESERVATION_BYTES, |context, _| {
        let root = installation(107, context)?;
        let external = tempfile::tempdir()?;
        let source = external.path().join("setup.exe");
        let archive = archive();
        for certificate in [None, Some(false), Some(true)] {
            let bytes = envelope(&archive, certificate);
            std::fs::write(&source, &bytes)?;
            let mut download =
                Download::open(storage(root.path(), context)?, signed(&archive)?, context)?;
            assert!(
                download
                    .receive(
                        Response {
                            status: 200,
                            length: Some(archive.len() as u64),
                            range: None,
                            encoding: None
                        },
                        &mut Cursor::new(&archive[..5]),
                        context,
                    )
                    .is_err()
            );
            drop(download);
            let mut verified = Download::import_installer(
                storage(root.path(), context)?,
                signed(&archive)?,
                &source,
                || {
                    assert!(std::fs::write(&source, b"changed").is_err());
                    assert!(std::fs::remove_file(&source).is_err());
                    NOW
                },
                context,
            )?;
            let mut actual = Vec::new();
            verified.read_to_end(&mut actual)?;
            assert_eq!(actual, archive);
            assert_eq!(std::fs::read(&source)?, bytes);
            std::fs::write(&source, b"caller retains the installer")?;
        }
        Ok(())
    })
}

#[test]
fn malformed_installer_ranges_preserve_partial_cache() -> TestResult {
    worker::run_reserved(DOWNLOAD_RESERVATION_BYTES, |context, _| {
        let root = installation(108, context)?;
        let external = tempfile::tempdir()?;
        let source = external.path().join("setup.exe");
        let archive = archive();
        let mut download =
            Download::open(storage(root.path(), context)?, signed(&archive)?, context)?;
        assert!(
            download
                .receive(
                    Response {
                        status: 200,
                        length: Some(archive.len() as u64),
                        range: None,
                        encoding: None
                    },
                    &mut Cursor::new(&archive[..5]),
                    context,
                )
                .is_err()
        );
        drop(download);
        let mut short = envelope(&archive, None);
        let footer = short.len() - 24;
        short[footer..footer + 8].copy_from_slice(&1_u64.to_le_bytes());
        let mut oversized = envelope(&archive, None);
        oversized[footer + 8..footer + 16].copy_from_slice(&u64::MAX.to_le_bytes());
        let mut truncated = envelope(&archive, Some(false));
        truncated.pop();
        let mut padding = envelope(&archive, Some(false));
        let padding_start = 512 + archive.len();
        assert_ne!(padding_start % 8, 0);
        padding[padding_start] = 1;
        for bytes in [b"no footer".to_vec(), archive.clone(), short, oversized, truncated, padding]
        {
            std::fs::write(&source, &bytes)?;
            assert!(
                Download::import_installer(
                    storage(root.path(), context)?,
                    signed(&archive)?,
                    &source,
                    || NOW,
                    context
                )
                .is_err()
            );
            let mut cached =
                Download::open(storage(root.path(), context)?, signed(&archive)?, context)?;
            assert_eq!(cached.offset()?, 5);
            cached.custody.file.seek(SeekFrom::Start(0))?;
            let mut prefix = Vec::new();
            cached.custody.file.read_to_end(&mut prefix)?;
            assert_eq!(prefix, archive[..5]);
            assert_eq!(std::fs::read(&source)?, bytes);
        }
        Ok(())
    })
}

#[test]
fn installer_footer_and_certificate_cannot_authorize_changed_package_bytes() -> TestResult {
    worker::run_reserved(DOWNLOAD_RESERVATION_BYTES, |context, _| {
        let root = installation(109, context)?;
        let external = tempfile::tempdir()?;
        let source = external.path().join("setup.exe");
        let archive = archive();
        let mut bytes = envelope(&archive, Some(false));
        bytes[512] ^= 1;
        std::fs::write(&source, &bytes)?;
        assert!(matches!(
            Download::import_installer(
                storage(root.path(), context)?,
                signed(&archive)?,
                &source,
                || NOW,
                context
            ),
            Err(DownloadFailure::Verification(Verification::Digest))
        ));
        assert_eq!(
            Download::open(storage(root.path(), context)?, signed(&archive)?, context)?.offset()?,
            0
        );
        assert_eq!(std::fs::read(&source)?, bytes);
        Ok(())
    })
}
