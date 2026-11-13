// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/**
 * The key ceremony writes private keys only outside the checkout, the public trust files at
 * the paths release builds read, and secret commands that never carry a key in arguments.
 */
import assert from 'node:assert/strict';
import { createHash, createPublicKey } from 'node:crypto';
import fs from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';
import {
  ceremony,
  releaseAnchorPath,
  secretCommand,
  secrets,
  trustedRootPath,
} from '../key-ceremony.mjs';

const repository = fileURLToPath(new URL('../../../', import.meta.url));
const signatureModule = 'Substitute/installer/release/release-signature.mjs';

/** Stand-in for the signing tool: keygen writes a seed, init writes the first repository. */
function fakeTool(calls) {
  return (tool, args) => {
    calls.push(args);
    if (args[0] === 'keygen') {
      fs.writeFileSync(args[2], 'ab'.repeat(32) + '\n', { flag: 'wx' });
      return `${createHash('sha256').update(args[2]).digest('hex')}\n`;
    }
    if (args[0] === 'init') {
      for (const name of ['1.root.json', '1.snapshot.json', '1.targets.json', 'timestamp.json']) {
        fs.writeFileSync(path.join(args[2], name), `{"name":"${name}"}`);
      }
      return 'root 1 1\nwrote 1.root.json\n';
    }
    throw new Error(`unexpected command ${args[0]}`);
  };
}

describe('key ceremony', () => {
  let folder;
  let checkout;
  let options;
  let calls;
  beforeEach(() => {
    folder = fs.mkdtempSync(path.join(tmpdir(), 'key-ceremony-'));
    checkout = path.join(folder, 'checkout');
    fs.mkdirSync(path.join(checkout, path.dirname(signatureModule)), { recursive: true });
    fs.copyFileSync(path.join(repository, signatureModule), path.join(checkout, signatureModule));
    calls = [];
    options = {
      tool: process.execPath,
      keys: path.join(folder, 'private'),
      publish: path.join(folder, 'public'),
      checkout,
      now: 1794598540,
      log: () => {},
      execute: fakeTool(calls),
    };
  });
  afterEach(() => fs.rmSync(folder, { recursive: true, force: true }));

  it('generates every key, the trusted root and the release anchor', async () => {
    const { record, commands } = await ceremony(options);
    assert.deepEqual(
      calls.map((args) => args[0]),
      ['keygen', 'keygen', 'keygen', 'keygen', 'init'],
    );
    const init = calls.at(-1);
    assert.equal(init[init.indexOf('--now') + 1], '1794598540');
    for (const secret of secrets) {
      assert.ok(fs.statSync(path.join(options.keys, secret.file)).size > 0, secret.name);
    }
    assert.equal(
      fs.readFileSync(path.join(checkout, trustedRootPath), 'utf8'),
      '{"name":"1.root.json"}',
    );
    const anchor = fs.readFileSync(path.join(checkout, releaseAnchorPath));
    assert.equal(anchor.length, 411);
    assert.equal(anchor.subarray(0, 4).toString('latin1'), 'RSA1');
    const privateKey = fs.readFileSync(path.join(options.keys, 'release-manifest.key.pem'), 'utf8');
    const modulus = Buffer.from(
      createPublicKey(privateKey).export({ format: 'jwk' }).n,
      'base64url',
    );
    assert.deepEqual(anchor.subarray(27), modulus);
    assert.equal(
      record.identities['release-manifest'],
      createHash('sha256').update(anchor).digest('hex'),
    );
    assert.deepEqual(record.initial_repository, [
      '1.root.json',
      '1.snapshot.json',
      '1.targets.json',
      'timestamp.json',
    ]);
    assert.deepEqual(
      commands,
      secrets.map((secret) =>
        secretCommand(
          'Artificial-Sweetener/SugarSubstitute-Rust',
          secret.name,
          path.join(options.keys, secret.file),
        ),
      ),
    );
    for (const command of commands) {
      assert.match(
        command,
        /^Get-Content -Raw -LiteralPath '.+' \| gh secret set [A-Z_]+ --env release --repo /,
      );
      assert.ok(!command.includes('ab'.repeat(32)));
      assert.ok(!command.includes('PRIVATE KEY'));
    }
  });

  it('names exactly the secrets the release and update repository workflows read', () => {
    const workflows = ['release.yml', 'substitute-update-repository.yml']
      .map((name) => fs.readFileSync(path.join(repository, '.github/workflows', name), 'utf8'))
      .join('\n');
    const read = new Set([...workflows.matchAll(/secrets\.([A-Z_]+)/g)].map((match) => match[1]));
    for (const ignored of ['GITHUB_TOKEN', 'REGISTRY_ACCESS_TOKEN', 'UPDATE_NEXT_ROOT_KEY']) {
      read.delete(ignored);
    }
    assert.deepEqual([...read].sort(), secrets.map((secret) => secret.name).sort());
  });

  it('refuses private keys inside the checkout or this repository', async () => {
    for (const keys of [
      path.join(checkout, 'keys'),
      path.join(repository, 'keys-ceremony-probe'),
    ]) {
      await assert.rejects(ceremony({ ...options, keys }), /outside the repository checkout/);
      assert.equal(fs.existsSync(keys), false);
    }
    await assert.rejects(ceremony({ ...options, keys: folder }), /must not contain a repository/);
    assert.equal(calls.length, 0);
  });

  it('refuses relative paths, used outputs and an existing trust root', async () => {
    await assert.rejects(ceremony({ ...options, keys: 'private' }), /absolute/);
    fs.mkdirSync(options.keys);
    fs.writeFileSync(path.join(options.keys, 'old.key'), 'x');
    await assert.rejects(ceremony(options), /new or empty/);
    fs.rmSync(options.keys, { recursive: true });
    fs.mkdirSync(path.dirname(path.join(checkout, trustedRootPath)), { recursive: true });
    fs.writeFileSync(path.join(checkout, trustedRootPath), '{}');
    await assert.rejects(ceremony(options), /rotation/);
    assert.equal(fs.existsSync(options.keys), false);
    assert.equal(calls.length, 0);
  });

  it('keeps public and private outputs apart', async () => {
    await assert.rejects(
      ceremony({ ...options, publish: path.join(options.keys, 'public') }),
      /separate directories/,
    );
  });
});
