// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/**
 * Create every signing key a SugarSubstitute release needs, once, offline.
 *
 * The update repository's role keys come from its own signing tool (`keygen`), the release
 * manifest key from the release build tooling. The tool's `init` signs the first repository
 * state with the D5 lifetimes. Public results land in the checkout at the paths release builds
 * read: the trusted root at `.github/update-repository/root.json` and the release-manifest
 * anchor at `Substitute/installer/release/release-anchor.bin`. The first repository state goes
 * to `--publish`, to be uploaded to the update repository release. Private keys go only to
 * `--keys`, which must lie outside the checkout. The script prints the commands that install
 * each key as a `release` environment secret, reading the value from its file.
 *
 * Usage: node .github/release/key-ceremony.mjs --tool <sugarsubstitute-update-repository>
 *          --keys <new private directory> --publish <new public directory>
 *          [--checkout <repository checkout>] [--repo <owner/name>] [--now <unix seconds>]
 *
 * SIGNING-KEYS.md lists every secret, the job that reads it, its lifetime and its rotation.
 */
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const scriptCheckout = fileURLToPath(new URL('../../', import.meta.url));

/** Every secret the release and update repository workflows read, with its file and role. */
export const secrets = [
  {
    name: 'UPDATE_ROOT_KEY',
    file: 'update-root.key',
    role: 'root',
    purpose: 'Signs root.json: the update repository trust anchor installations ship with.',
  },
  {
    name: 'UPDATE_TARGETS_KEY',
    file: 'update-targets.key',
    role: 'targets',
    purpose: 'Signs targets.json: the packages and channel descriptors offered.',
  },
  {
    name: 'UPDATE_SNAPSHOT_KEY',
    file: 'update-snapshot.key',
    role: 'snapshot',
    purpose: 'Signs snapshot.json: the consistent set of targets metadata.',
  },
  {
    name: 'UPDATE_TIMESTAMP_KEY',
    file: 'update-timestamp.key',
    role: 'timestamp',
    purpose: 'Signs timestamp.json, re-signed daily.',
  },
  {
    name: 'RELEASE_MANIFEST_KEY',
    file: 'release-manifest.key.pem',
    role: 'release-manifest',
    purpose:
      'Signs sugarsubstitute-release-v1.manifest, which binds the shipped host and crash handler images.',
  },
];

export const trustedRootPath = '.github/update-repository/root.json';
export const releaseAnchorPath = 'Substitute/installer/release/release-anchor.bin';

/** True when `candidate` is `root` or lies inside it, compared case-insensitively on Windows. */
function within(root, candidate) {
  const fold = (value) => (process.platform === 'win32' ? value.toLowerCase() : value);
  const relative = path.relative(fold(path.resolve(root)), fold(path.resolve(candidate)));
  return relative === '' || (!relative.startsWith('..') && !path.isAbsolute(relative));
}

/** Resolve through existing links so a junction into the checkout is refused too. */
function real(candidate) {
  let existing = path.resolve(candidate);
  const rest = [];
  while (!fs.existsSync(existing)) {
    rest.unshift(path.basename(existing));
    const parent = path.dirname(existing);
    if (parent === existing) break;
    existing = parent;
  }
  return path.join(fs.realpathSync.native(existing), ...rest);
}

/** Create a directory that did not exist before, or accept an existing empty one. */
function freshDirectory(directory, label) {
  if (fs.existsSync(directory)) {
    if (!fs.statSync(directory).isDirectory() || fs.readdirSync(directory).length) {
      throw new Error(`${label} ${directory} must be a new or empty directory`);
    }
  } else {
    fs.mkdirSync(directory, { recursive: true });
  }
}

function parse(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index];
    if (!name?.startsWith('--') || argv[index + 1] === undefined) {
      throw new Error(`Unexpected argument ${name}`);
    }
    const key = name.slice(2);
    if (!['tool', 'keys', 'publish', 'checkout', 'repo', 'now'].includes(key)) {
      throw new Error(`Unknown option ${name}`);
    }
    if (key in options) throw new Error(`${name} is given twice`);
    options[key] = argv[index + 1];
  }
  for (const required of ['tool', 'keys', 'publish']) {
    if (!options[required]) throw new Error(`--${required} is required`);
  }
  return options;
}

/** Run one signing tool command and return its standard output. */
function runTool(tool, args) {
  const result = spawnSync(tool, args, { encoding: 'utf8', windowsHide: true });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${path.basename(tool)} ${args[0]} failed: ${result.stderr.trim()}`);
  }
  return result.stdout;
}

/** PowerShell installs each secret from its file; the value never appears in arguments. */
export function secretCommand(repo, name, file) {
  const quoted = `'${file.replaceAll("'", "''")}'`;
  return `Get-Content -Raw -LiteralPath ${quoted} | gh secret set ${name} --env release --repo ${repo}`;
}

/**
 * Run the ceremony. Refuses before writing anything when a private path lies in a checkout,
 * when an output already holds files, or when the checkout already has a root or anchor.
 */
export async function ceremony({
  tool,
  keys,
  publish,
  checkout = scriptCheckout,
  repo = 'Artificial-Sweetener/SugarSubstitute-Rust',
  now = Math.floor(Date.now() / 1000),
  log = console.log,
  execute = runTool,
}) {
  for (const [label, value] of [
    ['--tool', tool],
    ['--keys', keys],
    ['--publish', publish],
    ['--checkout', checkout],
  ]) {
    if (!path.isAbsolute(value)) throw new Error(`${label} must be an absolute path`);
  }
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repo)) throw new Error('--repo is owner/name');
  if (!Number.isSafeInteger(Number(now)) || Number(now) <= 0) {
    throw new Error('--now is Unix UTC seconds');
  }
  const privateRoot = real(keys);
  for (const repository of new Set([real(scriptCheckout), real(checkout)])) {
    if (within(repository, privateRoot)) {
      throw new Error(`Private keys must stay outside the repository checkout ${repository}`);
    }
    if (within(privateRoot, repository)) {
      throw new Error('The private key directory must not contain a repository checkout');
    }
  }
  if (within(privateRoot, real(publish)) || within(real(publish), privateRoot)) {
    throw new Error('Public and private outputs must be separate directories');
  }
  const root = path.join(checkout, trustedRootPath);
  const anchor = path.join(checkout, releaseAnchorPath);
  for (const existing of [root, anchor]) {
    if (fs.existsSync(existing)) {
      throw new Error(
        `${existing} already exists; a new root is a rotation (rotate-root), not a ceremony`,
      );
    }
  }
  if (!fs.existsSync(tool) || !fs.statSync(tool).isFile()) {
    throw new Error('--tool is the built update repository signing tool');
  }
  freshDirectory(keys, 'The private key directory');
  freshDirectory(publish, 'The public output directory');

  const run = (...args) => execute(tool, args);
  const identities = {};
  for (const secret of secrets.filter((entry) => entry.role !== 'release-manifest')) {
    const file = path.join(keys, secret.file);
    identities[secret.role] = run('keygen', '--output', file).trim();
  }
  const signature = await import(
    pathToFileURL(path.join(checkout, 'Substitute/installer/release/release-signature.mjs')).href
  );
  const manifestKey = signature.generateReleaseKey();
  const manifestFile = path.join(keys, 'release-manifest.key.pem');
  fs.writeFileSync(manifestFile, manifestKey.privatePem, { flag: 'wx', mode: 0o600 });
  identities['release-manifest'] = signature.keyId(manifestKey.publicBlob);

  const keyArgument = (role) =>
    `file:${path.join(keys, secrets.find((entry) => entry.role === role).file)}`;
  const initialized = run(
    'init',
    '--repository',
    publish,
    '--now',
    String(now),
    '--root-key',
    keyArgument('root'),
    '--targets-key',
    keyArgument('targets'),
    '--snapshot-key',
    keyArgument('snapshot'),
    '--timestamp-key',
    keyArgument('timestamp'),
  );
  fs.mkdirSync(path.dirname(root), { recursive: true });
  fs.copyFileSync(path.join(publish, '1.root.json'), root, fs.constants.COPYFILE_EXCL);
  fs.writeFileSync(anchor, manifestKey.publicBlob, { flag: 'wx' });

  const sha256 = (file) => createHash('sha256').update(fs.readFileSync(file)).digest('hex');
  const record = {
    schema_version: 1,
    created: now,
    repository: repo,
    identities,
    trusted_root: { path: trustedRootPath, sha256: sha256(root) },
    release_anchor: { path: releaseAnchorPath, sha256: sha256(anchor) },
    initial_repository: fs.readdirSync(publish).sort(),
    secrets: secrets.map(({ name, role, purpose }) => ({ name, role, purpose })),
  };
  fs.writeFileSync(path.join(keys, 'ceremony.json'), JSON.stringify(record, null, 2) + '\n', {
    flag: 'wx',
  });

  log(initialized.trimEnd());
  log(`Trusted root: ${root}`);
  log(`Release manifest anchor: ${anchor} (key ${identities['release-manifest']})`);
  log(`Initial update repository to upload to the substitute-updates release: ${publish}`);
  log('Install each secret in the release environment (PowerShell, from this checkout):');
  const commands = secrets.map((secret) =>
    secretCommand(repo, secret.name, path.join(keys, secret.file)),
  );
  for (const command of commands) log(command);
  return { record, commands };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const options = parse(process.argv.slice(2));
  await ceremony({ ...options, ...(options.now ? { now: Number(options.now) } : {}) });
}
