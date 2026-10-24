// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Read the producing engine identity without inferring it from release labels or entry versions. */
import fs from 'node:fs/promises'
import { regularArtifact } from './artifact-files.mjs'

export const engineBuildFile = 'sugarsubstitute-engine-build.json'

/** The canonical build record is bounded and uses decimal text for the full Rust u64 range.
 * This is build provenance, not signature verification or runtime execution authority.
 */
export async function readEngineIdentity(file) {
  if (typeof file !== 'string') throw new Error('Producing engine identity is absent')
  if ((await regularArtifact(file)).size > 512) throw new Error('Producing engine identity is oversized')
  const handle = await fs.open(file, 'r')
  let bytes
  try {
    const buffer = Buffer.alloc(513)
    const { bytesRead } = await handle.read(buffer, 0, buffer.length, 0)
    if (bytesRead > 512) throw new Error('Producing engine identity is oversized')
    bytes = buffer.subarray(0, bytesRead).toString('utf8')
  } finally { await handle.close() }
  const identity = JSON.parse(bytes)
  if (identity?.schema_version !== 1 || typeof identity.release !== 'string' || !/^[1-9][0-9]{0,19}$/.test(identity.release) || BigInt(identity.release) > 18446744073709551615n || !Number.isInteger(identity.protocol) || identity.protocol < 1 || identity.protocol > 4294967295) throw new Error('Invalid producing engine identity')
  const canonical = { schema_version: 1, release: identity.release, protocol: identity.protocol }
  if (bytes !== JSON.stringify(canonical) + '\n') throw new Error('Producing engine identity must be canonical and contain only supported fields')
  return canonical
}
