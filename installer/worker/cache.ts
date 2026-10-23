// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Own reusable download entries. Automatic cleanup never traverses unknown cache content. */
import fs from 'node:fs'
import path from 'node:path'
import { createHash } from 'node:crypto'
import type { CacheGrant } from './cache-policy'
import type { Cache } from '@upstream/lib/cache'

/** Downloads retain upstream verification and resume metadata; this owner controls their storage. */
export function downloadCache(grant: CacheGrant): Cache {
  const root = grant.root
  function getCachePath(key: string): string {
    if (!key || key.length > 1024) throw new Error('Invalid upstream cache identity')
    const identity = createHash('sha256').update(key).digest('hex')
    const entry = path.join(root, identity)
    const marker = path.join(entry, '.substitute-cache.json')
    const record = JSON.stringify({ schema_version: 1, upstreamKey: key })
    grant.reserve(marker, Buffer.byteLength(record))
    if (!fs.existsSync(marker) && fs.readdirSync(entry).length) {
      throw new Error('Download cache contains unowned files')
    }
    try { fs.writeFileSync(marker, record, { flag: 'wx' }) } catch (cause) {
      if ((cause as NodeJS.ErrnoException).code !== 'EEXIST') throw cause
      const metadata = fs.lstatSync(marker)
      if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size > 8192 ||
          fs.readFileSync(marker, 'utf8') !== record) throw new Error('Download cache identity changed')
    }
    grant.settled(marker)
    return entry
  }
  return {
    getCachePath,
    touch(key) {
      const entry = getCachePath(key)
      const now = new Date()
      fs.utimesSync(entry, now, now)
    },
    // Shared native storage retires complete grants after actual worker settlement.
    evict() { grant.validate() },
    async cleanPartials() { grant.validate() },
  }
}
