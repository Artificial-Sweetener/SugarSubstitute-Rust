// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A dropped connection resumes within the same campaign from its exact durable package prefix.
use super::*;

#[test]
fn disconnected_download_continues_in_the_same_worker_and_prepares_the_same_package() -> TestResult
{
    let mut pipeline = Pipeline::new()?;
    let (fixture, signed) = initialized(&mut pipeline)?;
    let name = format!("{:x}.ssupdate", Sha256::digest(&fixture.archive));
    let total = fixture.archive.len();
    let mut exchanges = discovery(&signed)?;
    exchanges.push(exchange(
        &name,
        Some("bytes=0-"),
        200,
        &format!("Content-Length: {total}\r\n"),
        &fixture.archive[..7],
    ));
    exchanges.push(exchange(
        &name,
        Some("bytes=7-"),
        206,
        &format!("Content-Length: 4\r\nContent-Range: bytes 7-10/{total}\r\n"),
        &fixture.archive[7..11],
    ));
    exchanges.push(exchange(
        &name,
        Some("bytes=11-"),
        206,
        &format!(
            "Content-Length: {}\r\nContent-Range: bytes 11-{}/{total}\r\n",
            total - 11,
            total - 1
        ),
        &fixture.archive[11..],
    ));
    let (server, policy) =
        pipeline.run(8 * 1024 * 1024, move |context| Running::start(exchanges, &context))?;
    let client = (|| {
        let mut resumed = Worker::new(configuration(&fixture, policy))?;
        step(&mut pipeline, &mut resumed)?;
        step(&mut pipeline, &mut resumed)?;
        step(&mut pipeline, &mut resumed)?;
        // The dropped first response is progress, not a failed campaign: no rediscovery follows.
        for received in [7, 11] {
            let expected = Progress::Downloading { received, total: total as u64 };
            let observed = step(&mut pipeline, &mut resumed)?;
            if observed.progress != expected || observed.failure.is_some() {
                return Err(format!(
                    "Resumed download progress {:?}, expected {expected:?}",
                    observed.progress
                )
                .into());
            }
        }
        for expected in [Progress::Preparing, Progress::Ready] {
            let actual = step(&mut pipeline, &mut resumed)?.progress;
            if actual != expected {
                return Err(
                    format!("Resumed worker progress {actual:?}, expected {expected:?}").into()
                );
            }
        }
        retire(&mut pipeline, &mut resumed)?;
        assert_unlocked(&mut pipeline, &fixture)?;
        Ok(())
    })();
    server.finish_after(client)
}
