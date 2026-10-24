// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Cancellation and cache damage must preserve unrelated files and never produce authority.
use super::*;

#[test]
fn incomplete_finish_preserves_resumable_bytes_and_excess_response_bytes_are_rejected() -> TestResult
{
    worker::run_reserved(DOWNLOAD_RESERVATION_BYTES, |context, _| {
        let root = installation(84, context)?;
        let mut download = Download::open(root.path(), target()?, context)?;
        assert!(download.receive(full(), &mut Cursor::new(&PACKAGE[..6]), context).is_err());
        assert!(matches!(
            download.finish(|| NOW, context),
            Err(DownloadFailure::Response(Rejection::BodyLength))
        ));
        let mut download = Download::open(root.path(), target()?, context)?;
        assert_eq!(download.offset()?, 6);
        let oversized = [PACKAGE, b"extra"].concat();
        assert!(matches!(
            download.receive(full(), &mut Cursor::new(oversized), context),
            Err(DownloadFailure::Response(Rejection::BodyLength))
        ));
        assert_eq!(download.offset()?, 0);
        Ok(())
    })
}

#[test]
fn oversized_owned_cache_is_an_observable_miss_and_unknown_files_survive() -> TestResult {
    worker::run_reserved(DOWNLOAD_RESERVATION_BYTES, |context, _| {
        let root = installation(85, context)?;
        drop(Download::open(root.path(), target()?, context)?);
        let path = payload(root.path())?;
        let unknown = path.with_file_name("user-file");
        std::fs::write(&unknown, b"preserve")?;
        std::fs::write(&path, vec![7; PACKAGE.len() + 1])?;
        let download = Download::open(root.path(), target()?, context)?;
        assert!(download.cache_was_reset());
        assert_eq!(download.offset()?, 0);
        assert_eq!(std::fs::read(unknown)?, b"preserve");
        Ok(())
    })
}

#[test]
fn expiry_during_hashing_withholds_authority_but_retains_cached_bytes() -> TestResult {
    worker::run_reserved(DOWNLOAD_RESERVATION_BYTES, |context, _| {
        let root = installation(86, context)?;
        let mut download = Download::open(root.path(), target()?, context)?;
        download.receive(full(), &mut Cursor::new(PACKAGE), context)?;
        let mut time = NOW;
        assert!(matches!(
            download.finish(
                || {
                    let result = time;
                    time = EXPIRES;
                    result
                },
                context
            ),
            Err(DownloadFailure::Verification(Verification::Expired))
        ));
        assert_eq!(std::fs::read(payload(root.path())?)?, PACKAGE);
        Ok(())
    })
}

#[test]
fn ambiguous_ownership_is_preserved_without_truncating_unknown_payload() -> TestResult {
    worker::run_reserved(DOWNLOAD_RESERVATION_BYTES, |context, _| {
        let root = installation(87, context)?;
        drop(Download::open(root.path(), target()?, context)?);
        let path = payload(root.path())?;
        std::fs::write(&path, b"unknown")?;
        std::fs::write(path.with_file_name("owner"), b"damaged")?;
        assert!(Download::open(root.path(), target()?, context).is_err());
        assert_eq!(std::fs::read(&path)?, b"unknown");
        assert_eq!(std::fs::read(path.with_file_name("owner"))?, b"damaged");
        Ok(())
    })
}

#[test]
fn cancelled_transfer_flushes_earlier_prefix_and_does_not_commit_new_bytes() -> TestResult {
    worker::run_reserved_expiring(DOWNLOAD_RESERVATION_BYTES, |context, clock| {
        struct Expire<'a>(&'a worker::TestClock);
        impl Read for Expire<'_> {
            fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
                bytes[0] = b'x';
                self.0.expire();
                Ok(1)
            }
        }
        let root = installation(88, context)?;
        let mut download = Download::open(root.path(), target()?, context)?;
        assert!(download.receive(full(), &mut Cursor::new(&PACKAGE[..5]), context).is_err());
        let range = format!("bytes 5-{}/{}", PACKAGE.len() - 1, PACKAGE.len());
        assert!(matches!(
            download.receive(
                Response { status: 206, length: None, range: Some(&range), encoding: None },
                &mut Expire(clock),
                context
            ),
            Err(DownloadFailure::Storage(_))
        ));
        drop(download);
        assert_eq!(std::fs::read(payload(root.path())?)?, &PACKAGE[..5]);
        Ok(())
    })
}
