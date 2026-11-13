// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real cache files exercise resume, restart and verified-byte custody through admitted jobs.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult, installation};
use std::io::Cursor;
use sugarsubstitute_update::{Error as Verification, RepositoryTrust, fixture::*};
mod failures;
mod installer;
mod local;
const PACKAGE: &[u8] = b"native update package fixture";

fn target() -> Result<Target, Box<dyn std::error::Error + Send + Sync>> {
    let key = key(63)?;
    let mut trust = RepositoryTrust::bootstrap(&root(&key, 1, None)?)?;
    trust.refresh(Set::new(&key, [1; 3], PACKAGE, EXPIRES)?.metadata(), NOW)?;
    Ok(trust.target(NAME, NOW)?)
}
fn full() -> Response<'static> {
    Response { status: 200, length: Some(PACKAGE.len() as u64), range: None, encoding: None }
}
/// Create the same explicit storage owner supplied by installation trust in production.
fn storage(root: &Path, context: &JobContext) -> io::Result<RuntimeCache> {
    RuntimeCache::open(root, crate::setup_storage::cache::CachePolicy::default(), context)
}
/// Inspect the one known fixture generation; the returned path grants no runtime custody.
fn payload(root: &Path) -> Result<std::path::PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    Ok(worker::cache_file(
        root,
        crate::setup_storage::cache::Namespace::UpdateDownloads,
        "payload",
    )?)
}

/// A network boundary that returns a prefix, then loses the connection deterministically.
struct Disconnect(Cursor<Vec<u8>>);
impl Read for Disconnect {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let count = self.0.read(output)?;
        if count == 0 { Err(io::Error::from(io::ErrorKind::ConnectionReset)) } else { Ok(count) }
    }
}

#[test]
fn interrupted_download_reopens_at_actual_offset_and_verifies_the_entire_file() -> TestResult {
    worker::run_reserved(2 * DOWNLOAD_RESERVATION_BYTES, |context, _| {
        let root = installation(80, context)?;
        let prefix = 7;
        {
            let mut download = Download::open(storage(root.path(), context)?, target()?, context)?;
            assert!(matches!(
                download.receive(
                    full(),
                    &mut Disconnect(Cursor::new(PACKAGE[..prefix].to_vec())),
                    context
                ),
                Err(DownloadFailure::Storage(_))
            ));
            assert_eq!(download.offset()?, prefix as u64);
        }
        let mut download = Download::open(storage(root.path(), context)?, target()?, context)?;
        assert_eq!(download.offset()?, prefix as u64);
        let range = format!("bytes {prefix}-{}/{}", PACKAGE.len() - 1, PACKAGE.len());
        download.receive(
            Response {
                status: 206,
                length: Some((PACKAGE.len() - prefix) as u64),
                range: Some(&range),
                encoding: None,
            },
            &mut Cursor::new(&PACKAGE[prefix..]),
            context,
        )?;
        let mut verified = download.finish(|| NOW, context)?;
        assert_eq!(verified.identity().length(), PACKAGE.len() as u64);
        let mut actual = Vec::new();
        verified.read_to_end(&mut actual)?;
        assert_eq!(actual, PACKAGE);
        assert!(Download::open(storage(root.path(), context)?, target()?, context).is_err());
        assert!(std::fs::write(payload(root.path())?, b"changed").is_err());
        assert!(std::fs::remove_file(payload(root.path())?).is_err());
        drop(verified);
        Download::open(storage(root.path(), context)?, target()?, context)?
            .finish(|| NOW, context)?;
        Ok(())
    })
}

#[test]
fn ignored_range_restarts_the_owned_payload_instead_of_appending() -> TestResult {
    worker::run_reserved(DOWNLOAD_RESERVATION_BYTES, |context, _| {
        let root = installation(81, context)?;
        let mut download = Download::open(storage(root.path(), context)?, target()?, context)?;
        assert!(download.receive(full(), &mut Cursor::new(&PACKAGE[..5]), context).is_err());
        assert_eq!(download.offset()?, 5);
        download.receive(full(), &mut Cursor::new(PACKAGE), context)?;
        let mut verified = download.finish(|| NOW, context)?;
        let mut actual = Vec::new();
        verified.read_to_end(&mut actual)?;
        assert_eq!(actual, PACKAGE);
        Ok(())
    })
}

#[test]
fn malformed_framing_never_changes_the_retained_prefix_or_reads_the_body() -> TestResult {
    worker::run_reserved(DOWNLOAD_RESERVATION_BYTES, |context, _| {
        let root = installation(82, context)?;
        let mut download = Download::open(storage(root.path(), context)?, target()?, context)?;
        assert!(download.receive(full(), &mut Cursor::new(&PACKAGE[..5]), context).is_err());
        let mut body = Cursor::new(PACKAGE);
        let result = download.receive(
            Response { status: 206, length: Some(1), range: Some("bytes 0-0/1"), encoding: None },
            &mut body,
            context,
        );
        assert!(matches!(result, Err(DownloadFailure::Response(Rejection::Framing))));
        assert_eq!(body.position(), 0);
        assert_eq!(download.offset()?, 5);
        drop(download);
        assert_eq!(std::fs::read(payload(root.path())?)?, &PACKAGE[..5]);
        Ok(())
    })
}

#[test]
fn corrupt_resumed_prefix_is_detected_and_next_attempt_starts_cleanly() -> TestResult {
    worker::run_reserved(DOWNLOAD_RESERVATION_BYTES, |context, _| {
        let root = installation(83, context)?;
        let mut download = Download::open(storage(root.path(), context)?, target()?, context)?;
        assert!(download.receive(full(), &mut Cursor::new(b"wrong"), context).is_err());
        let range = format!("bytes 5-{}/{}", PACKAGE.len() - 1, PACKAGE.len());
        download.receive(
            Response { status: 206, length: None, range: Some(&range), encoding: None },
            &mut Cursor::new(&PACKAGE[5..]),
            context,
        )?;
        assert!(matches!(
            download.finish(|| NOW, context),
            Err(DownloadFailure::Verification(Verification::Digest))
        ));
        let mut retry = Download::open(storage(root.path(), context)?, target()?, context)?;
        assert_eq!(retry.offset()?, 0);
        retry.receive(full(), &mut Cursor::new(PACKAGE), context)?;
        retry.finish(|| NOW, context)?;
        Ok(())
    })
}

#[test]
fn server_limited_contiguous_ranges_accumulate_until_the_whole_package_verifies() -> TestResult {
    worker::run_reserved(DOWNLOAD_RESERVATION_BYTES, |context, _| {
        let root = installation(89, context)?;
        let mut download = Download::open(storage(root.path(), context)?, target()?, context)?;
        for offset in (0..PACKAGE.len()).step_by(4) {
            let end = (offset + 4).min(PACKAGE.len());
            let range = format!("bytes {offset}-{}/{}", end - 1, PACKAGE.len());
            download.receive(
                Response {
                    status: 206,
                    length: Some((end - offset) as u64),
                    range: Some(&range),
                    encoding: None,
                },
                &mut Cursor::new(&PACKAGE[offset..end]),
                context,
            )?;
            assert_eq!(download.offset()?, end as u64);
        }
        download.finish(|| NOW, context)?;
        Ok(())
    })
}
