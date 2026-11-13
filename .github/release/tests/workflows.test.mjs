// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/**
 * The release workflows publish nothing unless publication is switched on.
 *
 * `release.yml` runs only when dispatched, and only its `authorize` job may decide to publish:
 * every job that holds write access or a secret depends on that decision. The decision script
 * itself is run here, with the inputs GitHub would give it.
 */
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { after, before, describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';
import { parse } from 'yaml';

const WORKFLOWS = fileURLToPath(new URL('../../workflows/', import.meta.url));
const PUBLISH = "needs.authorize.outputs.publish == 'true'";

function workflow(name) {
  return parse(readFileSync(join(WORKFLOWS, name), 'utf8'));
}

/** Whether a job, its steps or its called workflow may write or read secrets. */
function privileged(job) {
  const text = JSON.stringify(job);
  return (
    Object.values(job.permissions ?? {}).includes('write') ||
    text.includes('secrets.') ||
    job.environment !== undefined ||
    (typeof job.uses === 'string' && job.uses !== './.github/workflows/release-tooling.yml')
  );
}

describe('release workflow', () => {
  const release = workflow('release.yml');
  let folder;
  before(() => {
    folder = mkdtempSync(join(tmpdir(), 'release-workflow-'));
  });
  after(() => rmSync(folder, { recursive: true, force: true }));

  /** Run the authorize script as the runner would and return its exit status and outputs. */
  function authorize(env) {
    const output = join(folder, `output-${Math.random().toString(16).slice(2)}`);
    writeFileSync(output, '');
    const [step] = release.jobs.authorize.steps;
    const result = spawnSync('bash', ['-e', '-c', step.run], {
      env: { PATH: process.env.PATH, GITHUB_OUTPUT: output, ...env },
      encoding: 'utf8',
    });
    return { status: result.status, output: readFileSync(output, 'utf8'), log: result.stdout };
  }

  it('runs only when dispatched, as a dry run unless asked otherwise', () => {
    assert.deepEqual(Object.keys(release.on), ['workflow_dispatch']);
    assert.equal(release.on.workflow_dispatch.inputs.dry_run.default, 'true');
    assert.deepEqual(release.permissions, { contents: 'read' });
    assert.deepEqual(release.jobs.authorize.steps[0].env, {
      DRY_RUN: '${{ inputs.dry_run }}',
      RELEASES_ENABLED: '${{ vars.RELEASES_ENABLED }}',
      REF: '${{ github.ref }}',
    });
  });

  it('publishes only when asked, on main, with RELEASES_ENABLED exactly "true"', () => {
    const main = 'refs/heads/main';
    for (const env of [
      { DRY_RUN: 'true', RELEASES_ENABLED: 'true', REF: main },
      { DRY_RUN: 'true', REF: main },
      { DRY_RUN: '', REF: main },
    ]) {
      assert.deepEqual(authorize(env), {
        status: 0,
        output: 'publish=false\n',
        log: 'Dry run: nothing is published.\n',
      });
    }
    for (const env of [
      { DRY_RUN: 'false', REF: main },
      { DRY_RUN: 'false', RELEASES_ENABLED: '', REF: main },
      { DRY_RUN: 'false', RELEASES_ENABLED: 'TRUE', REF: main },
      { DRY_RUN: 'false', RELEASES_ENABLED: 'yes', REF: main },
      { DRY_RUN: 'false', RELEASES_ENABLED: 'true', REF: 'refs/heads/lane/release' },
    ]) {
      const outcome = authorize(env);
      assert.notEqual(outcome.status, 0, JSON.stringify(env));
      assert.equal(outcome.output, '', JSON.stringify(env));
    }
    assert.deepEqual(authorize({ DRY_RUN: 'false', RELEASES_ENABLED: 'true', REF: main }), {
      status: 0,
      output: 'publish=true\n',
      log: '',
    });
  });

  it('lets only jobs behind the publication decision write, use secrets or call other workflows', () => {
    const privilegedJobs = Object.entries(release.jobs).filter(([, job]) => privileged(job));
    assert.deepEqual(privilegedJobs.map(([name]) => name).sort(), [
      'substitute',
      'substitute-verify',
      'sugarcubes',
    ]);
    for (const [name, job] of privilegedJobs) {
      assert.ok([job.needs].flat().includes('authorize'), name);
      assert.ok(String(job.if).includes(PUBLISH), name);
    }
    assert.ok(!privileged(release.jobs.authorize));
    assert.ok(!privileged(release.jobs.tooling));
  });

  it('passes the publication switch to every real release, which checks it again', () => {
    for (const name of ['sugarcubes', 'substitute']) {
      const step = release.jobs[name].steps.find((entry) => entry.name === 'Release');
      assert.match(step.run, /^node \.github\/release\/release\.mjs release \w+$/);
      assert.equal(step.env.RELEASES_ENABLED, '${{ vars.RELEASES_ENABLED }}');
    }
    const registry = release.jobs.sugarcubes.steps.find((entry) =>
      String(entry.run).includes('sugarcubes_registry.py publish'),
    );
    assert.match(registry.if, /vars\.RELEASES_ENABLED == 'true'/);
  });
});

describe('release tooling workflow', () => {
  const tooling = workflow('release-tooling.yml');

  it('holds read access only and no secrets', () => {
    assert.deepEqual(tooling.permissions, { contents: 'read' });
    for (const job of Object.values(tooling.jobs)) {
      assert.equal(privileged(job), false);
      const checkout = job.steps.find((step) => String(step.uses).startsWith('actions/checkout'));
      assert.equal(checkout.with['persist-credentials'], false);
      assert.equal(checkout.with['fetch-depth'], 0);
    }
  });

  it('dry-runs every due component and requires the version the plan predicted', () => {
    const step = tooling.jobs.tooling.steps.find((entry) =>
      String(entry.run).includes('--dry-run'),
    );
    assert.equal(step.shell, 'bash');
    assert.match(step.run, /grep -qx "version=\$\{!version_var\}"/);
  });
});

describe('update repository workflow', () => {
  const update = workflow('substitute-update-repository.yml');
  let folder;
  before(() => {
    folder = mkdtempSync(join(tmpdir(), 'update-workflow-'));
  });
  after(() => rmSync(folder, { recursive: true, force: true }));

  function authorize(env) {
    const output = join(folder, `output-${Math.random().toString(16).slice(2)}`);
    writeFileSync(output, '');
    const [step] = update.jobs.authorize.steps;
    const result = spawnSync('bash', ['-e', '-c', step.run], {
      env: {
        PATH: process.env.PATH,
        GITHUB_OUTPUT: output,
        MONTHLY_SCHEDULE: update.env.MONTHLY_SCHEDULE,
        ...env,
      },
      encoding: 'utf8',
    });
    return { status: result.status, output: readFileSync(output, 'utf8') };
  }

  it('re-signs the timestamp daily and every role monthly', () => {
    assert.deepEqual(
      update.on.schedule.map((entry) => entry.cron),
      ['17 3 * * *', update.env.MONTHLY_SCHEDULE],
    );
    assert.match(update.env.MONTHLY_SCHEDULE, /^\d+ \d+ 1 \* \*$/);
    const daily = { EVENT: 'schedule', SCHEDULE: '17 3 * * *', RELEASES_ENABLED: 'true' };
    assert.deepEqual(authorize(daily), {
      status: 0,
      output: 'operation=refresh-timestamp\npublish=true\n',
    });
    assert.deepEqual(authorize({ ...daily, SCHEDULE: update.env.MONTHLY_SCHEDULE }), {
      status: 0,
      output: 'operation=resign\npublish=true\n',
    });
    assert.deepEqual(
      authorize({ EVENT: 'workflow_call', REQUESTED: 'release', RELEASES_ENABLED: 'true' }),
      { status: 0, output: 'operation=release\npublish=true\n' },
    );
    assert.notEqual(
      authorize({ EVENT: 'workflow_dispatch', REQUESTED: 'delete', RELEASES_ENABLED: 'true' })
        .status,
      0,
    );
  });

  it('signs nothing while RELEASES_ENABLED is not "true"', () => {
    assert.deepEqual(authorize({ EVENT: 'schedule', SCHEDULE: '17 3 * * *' }), {
      status: 0,
      output: 'operation=refresh-timestamp\npublish=false\n',
    });
    for (const RELEASES_ENABLED of ['', 'TRUE', 'false']) {
      assert.notEqual(
        authorize({ EVENT: 'workflow_dispatch', REQUESTED: 'resign', RELEASES_ENABLED }).status,
        0,
      );
    }
    assert.deepEqual(update.permissions, { contents: 'read' });
    const privilegedJobs = Object.entries(update.jobs).filter(([, job]) => privileged(job));
    assert.deepEqual(
      privilegedJobs.map(([name]) => name),
      ['sign'],
    );
    assert.equal(update.jobs.sign.needs, 'authorize');
    assert.equal(update.jobs.sign.if, PUBLISH);
  });

  it('verifies against the shipped root and uploads the timestamp last', () => {
    const steps = update.jobs.sign.steps;
    const sign = steps.find((step) => step.name === 'Sign and verify');
    assert.match(sign.run, /--trusted-root "\$TRUSTED"/);
    assert.match(sign.run, /--written "\$RUNNER_TEMP\/written\.txt"/);
    const names = steps.map((step) => step.name);
    assert.ok(
      names.indexOf('Verify as an installation that trusted the published state') <
        names.indexOf(
          'Publish packages, then metadata, descriptors and the timestamp in signing order',
        ),
    );
    const publish = steps.at(-1).run;
    assert.ok(publish.indexOf('timestamp.json" ]') < publish.indexOf('upload "$file"'));
  });
});
