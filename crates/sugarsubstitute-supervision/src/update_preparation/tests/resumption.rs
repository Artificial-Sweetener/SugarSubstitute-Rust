// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A new campaign reauthenticates and resumes a failed worker's exact durable package prefix.
use super::*;

#[test]
fn disconnected_worker_retires_and_a_new_worker_resumes_then_prepares_the_same_package()
-> TestResult {
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
    exchanges.extend(discovery(&signed)?);
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
    let port = policy.asset("timestamp.json")?.port()?;
    let mut failed = Worker::new(configuration(&fixture, policy))?;
    step(&mut pipeline, &mut failed)?;
    step(&mut pipeline, &mut failed)?;
    submit(&mut pipeline, &mut failed)?;
    let observed = pipeline.observe(|| failed.poll())?;
    assert_eq!(observed.progress, Progress::Failed);
    assert!(matches!(observed.outcome, JobOutcome::Failure(_)));
    let cause = observed.failure.ok_or("Lost disconnect cause")?;
    assert!(cause.to_string().contains("transport"));
    assert!(failed.take_candidate().is_err());
    retire(&mut pipeline, &mut failed)?;
    assert_unlocked(&mut pipeline, &fixture)?;
    let mut resumed = Worker::new(configuration(
        &fixture,
        sugarsubstitute_update::http::Policy::loopback(port)?,
    ))?;
    step(&mut pipeline, &mut resumed)?;
    step(&mut pipeline, &mut resumed)?;
    assert_eq!(
        step(&mut pipeline, &mut resumed)?.progress,
        Progress::Downloading { received: 11, total: total as u64 }
    );
    for expected in [Progress::Preparing, Progress::Ready] {
        assert_eq!(step(&mut pipeline, &mut resumed)?.progress, expected);
    }
    server.finish()?;
    retire(&mut pipeline, &mut resumed)?;
    assert_unlocked(&mut pipeline, &fixture)?;
    Ok(())
}
