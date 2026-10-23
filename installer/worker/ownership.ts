// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Preserve provisioning schemas and recovery decisions over the retained native record owner. */
import path from 'node:path'
import { randomUUID } from 'node:crypto'
import { nativeRecords } from './records'

export async function reserve(directory: string): Promise<{ directory: string; owner: string }> {
  const storage = nativeRecords()
  await storage.open(directory, true)
  try {
    const { value: current } = await readRecord()
    if (current.schema_version !== 1 || current.product !== 'SugarSubstitute' || current.kind !== 'managed-comfy') {
      throw new Error('Managed installation ownership could not be verified')
    }
  } catch (cause) {
    if ((cause as NodeJS.ErrnoException).code !== 'ENOENT') throw cause
    for await (const _entry of storage.entries()) throw new Error('Choose an empty folder for the managed environment')
    await storage.publish(JSON.stringify({ schema_version: 1, product: 'SugarSubstitute', kind: 'managed-comfy' }))
  }
  const owner = randomUUID()
  const attempt = await storage.createChild(owner)
  await storage.publish(JSON.stringify({ schema_version: 1, owner, state: 'provisioning' }), owner)
  return { directory: attempt, owner }
}

async function readRecord(child?: string): Promise<{ value: Record<string, unknown>; revision: number }> {
  const snapshot = await nativeRecords().read(child)
  return { value: JSON.parse(snapshot.text), revision: snapshot.revision }
}

/** Discover a completed owned attempt, preserving interrupted and unknown directories. */
export async function ready(
  directory: string, plan?: Record<string, unknown>
): Promise<{ directory: string; owner: string; installation: Record<string, unknown> } | undefined> {
  const storage = nativeRecords()
  let resolved: string
  let marker: Record<string, unknown>
  try {
    const selected = await storage.open(directory, false)
    if (selected === undefined) return undefined
    resolved = selected
    marker = (await readRecord()).value
  } catch (cause) {
    if ((cause as NodeJS.ErrnoException).code === 'ENOENT') return undefined
    throw cause
  }
  if (marker.schema_version !== 1 || marker.product !== 'SugarSubstitute' || marker.kind !== 'managed-comfy') {
    throw new Error('Managed installation ownership could not be verified')
  }
  let latest: { directory: string; owner: string; installation: Record<string, unknown>; created: number } | undefined
  for await (const entry of storage.entries()) {
    if (!entry.directory || !/^[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}$/.test(entry.name)) continue
    const candidate = path.join(resolved, entry.name)
    let current: Record<string, unknown>
    try { current = (await readRecord(entry.name)).value } catch (cause) {
      console.warn('Preserving unreadable provisioning attempt', entry.name, (cause as Error).name)
      continue
    }
    const installation = current.installation
    if (current.schema_version !== 1 || current.owner !== entry.name || current.state !== 'ready' ||
        !installation || typeof installation !== 'object' || Array.isArray(installation)) continue
    const value = installation as Record<string, unknown>
    if (value.id !== entry.name || typeof value.installPath !== 'string' || !samePath(value.installPath, candidate)) continue
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
  const storage = nativeRecords()
  if (!samePath(reservation.directory, storage.child(reservation.owner))) throw new Error('Provisioning attempt folder changed')
  const { value: current, revision } = await readRecord(reservation.owner)
  if (current.schema_version !== 1 || current.owner !== reservation.owner ||
      typeof current.state !== 'string' ||
      !['provisioning', 'ready', 'cancelled', 'failed'].includes(current.state) ||
      !['provisioning', 'ready', 'cancelled', 'failed'].includes(state)) {
    throw new Error('Provisioning ownership or schema changed')
  }
  await storage.publish(JSON.stringify({ schema_version: 1, owner: reservation.owner, state, installation }), reservation.owner, revision)
}
