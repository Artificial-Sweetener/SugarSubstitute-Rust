// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Preserve each provisioning attempt in an owned container so retries never overwrite environments. */
import fs from 'node:fs/promises'
import path from 'node:path'
import { randomUUID } from 'node:crypto'

export async function canonicalParent(directory: string): Promise<string> {
  if (!path.isAbsolute(directory) || path.parse(directory).root === directory ||
      directory.split(/[\\/]/).some(part => part === '..' || part === '.')) {
    throw new Error('Choose an absolute installation folder below a drive or filesystem root')
  }
  const parent = await fs.realpath(path.dirname(directory))
  const requestedParent = path.resolve(path.dirname(directory))
  const equal = process.platform === 'win32'
    ? parent.toLowerCase() === requestedParent.toLowerCase() : parent === requestedParent
  if (!equal) throw new Error('Installation parent must not pass through a link or junction')
  return path.join(parent, path.basename(directory))
}

export async function reserve(directory: string): Promise<{ directory: string; owner: string }> {
  const resolved = await canonicalParent(directory)
  try { await fs.mkdir(resolved) } catch (cause) {
    if ((cause as NodeJS.ErrnoException).code !== 'EEXIST') throw cause
  }
  const metadata = await fs.lstat(resolved)
  if (!metadata.isDirectory() || metadata.isSymbolicLink()) throw new Error('Managed folder must be a real directory')
  const marker = path.join(resolved, '.substitute-managed.json')
  try {
    const current = await readRecord(marker)
    if (current.schema_version !== 1 || current.product !== 'SugarSubstitute' || current.kind !== 'managed-comfy') {
      throw new Error('Managed installation ownership could not be verified')
    }
  } catch (cause) {
    if ((cause as NodeJS.ErrnoException).code !== 'ENOENT') throw cause
    if ((await fs.readdir(resolved)).length) throw new Error('Choose an empty folder for the managed environment')
    await fs.writeFile(marker, JSON.stringify({
      schema_version: 1, product: 'SugarSubstitute', kind: 'managed-comfy',
    }), { flag: 'wx' })
  }
  const owner = randomUUID()
  const attempt = path.join(resolved, owner)
  await fs.mkdir(attempt)
  // If marker publication fails the folder stays untouched for explicit recovery.
  await fs.writeFile(path.join(attempt, '.substitute-provisioning.json'), JSON.stringify({
    schema_version: 1, owner, state: 'provisioning',
  }), { flag: 'wx' })
  return { directory: attempt, owner }
}

async function readRecord(file: string): Promise<Record<string, unknown>> {
  const metadata = await fs.lstat(file)
  if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size > 65536) {
    throw new Error('Provisioning record is not a bounded regular file')
  }
  return JSON.parse(await fs.readFile(file, 'utf8'))
}

/** Discover a completed owned attempt, preserving interrupted and unknown directories. */
export async function ready(
  directory: string, plan?: Record<string, unknown>
): Promise<{ directory: string; owner: string; installation: Record<string, unknown> } | undefined> {
  const resolved = await canonicalParent(directory)
  let marker: Record<string, unknown>
  try { marker = await readRecord(path.join(resolved, '.substitute-managed.json')) } catch (cause) {
    if ((cause as NodeJS.ErrnoException).code === 'ENOENT') return undefined
    throw cause
  }
  if (marker.schema_version !== 1 || marker.product !== 'SugarSubstitute' || marker.kind !== 'managed-comfy') {
    throw new Error('Managed installation ownership could not be verified')
  }
  const real = await fs.realpath(resolved)
  if (!samePath(real, resolved) || (await fs.lstat(resolved)).isSymbolicLink()) {
    throw new Error('Managed installation must not traverse links')
  }
  let latest: { directory: string; owner: string; installation: Record<string, unknown>; created: number } | undefined
  let count = 0
  for await (const entry of await fs.opendir(resolved)) {
    if (++count > 4096) throw new Error('Managed environment inventory exceeds its bound')
    if (!entry.isDirectory() || !/^[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}$/.test(entry.name)) continue
    const candidate = path.join(resolved, entry.name)
    let current: Record<string, unknown>
    try { current = await readRecord(path.join(candidate, '.substitute-provisioning.json')) } catch (cause) {
      console.warn('Preserving unreadable provisioning attempt', entry.name, (cause as Error).name)
      continue
    }
    const installation = current.installation
    if (current.schema_version !== 1 || current.owner !== entry.name || current.state !== 'ready' ||
        !installation || typeof installation !== 'object' || Array.isArray(installation)) continue
    const value = installation as Record<string, unknown>
    if (value.id !== entry.name || typeof value.installPath !== 'string' ||
        !samePath(value.installPath, candidate) || !samePath(await fs.realpath(candidate), candidate)) continue
    if (plan && !Object.entries(plan).every(([key, expected]) => JSON.stringify(value[key]) === JSON.stringify(expected))) continue
    const created = typeof value.createdAt === 'string' ? Date.parse(value.createdAt) : NaN
    if (!Number.isFinite(created)) continue
    if (!latest || created > latest.created || created === latest.created && entry.name > latest.owner) {
      latest = { directory: candidate, owner: entry.name, installation: value, created }
    }
  }
  return latest
}

function samePath(left: string, right: string): boolean {
  return process.platform === 'win32' ? left.toLowerCase() === right.toLowerCase() : left === right
}

export async function record(
  reservation: { directory: string; owner: string }, state: string, installation?: unknown
): Promise<void> {
  const file = path.join(reservation.directory, '.substitute-provisioning.json')
  const current = await readRecord(file)
  if (current.schema_version !== 1 || current.owner !== reservation.owner ||
      typeof current.state !== 'string' ||
      !['provisioning', 'ready', 'cancelled', 'failed'].includes(current.state) ||
      !['provisioning', 'ready', 'cancelled', 'failed'].includes(state)) {
    throw new Error('Provisioning ownership or schema changed')
  }
  const bytes = JSON.stringify({ schema_version: 1, owner: reservation.owner, state, installation })
  if (Buffer.byteLength(bytes) > 65536) throw new Error('Provisioning record exceeds its bound')
  const temporary = `${file}.${randomUUID()}.tmp`
  await fs.writeFile(temporary, bytes, { flag: 'wx' })
  await fs.rename(temporary, file)
}
