// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Own reusable download entries. Automatic cleanup never traverses unknown cache content. */
import fs from 'node:fs'
import path from 'node:path'
import { createHash } from 'node:crypto'
import type { Cache } from '@upstream/lib/cache'

/** Downloads retain upstream verification and resume metadata; this owner controls their storage. */
export function downloadCache(root: string): Cache {
  function directory(value: string): void {
    fs.mkdirSync(value, { recursive: true })
    const metadata = fs.lstatSync(value)
    const actual = fs.realpathSync(value)
    const requested = path.resolve(value)
    const same = process.platform === 'win32'
      ? actual.toLowerCase() === requested.toLowerCase() : actual === requested
    if (!metadata.isDirectory() || metadata.isSymbolicLink() || !same) {
      throw new Error('Download cache must not traverse links')
    }
  }
  function getCachePath(key: string): string {
    if (!key || key.length > 1024) throw new Error('Invalid upstream cache identity')
    directory(root)
    const identity = createHash('sha256').update(key).digest('hex')
    const entry = path.join(root, identity)
    directory(entry)
    const marker = path.join(entry, '.substitute-cache.json')
    const record = JSON.stringify({ schema_version: 1, upstreamKey: key })
    if (!fs.existsSync(marker) && fs.readdirSync(entry).length) {
      throw new Error('Download cache contains unowned files')
    }
    try { fs.writeFileSync(marker, record, { flag: 'wx' }) } catch (cause) {
      if ((cause as NodeJS.ErrnoException).code !== 'EEXIST') throw cause
      const metadata = fs.lstatSync(marker)
      if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size > 8192 ||
          fs.readFileSync(marker, 'utf8') !== record) throw new Error('Download cache identity changed')
    }
    return entry
  }
  return {
    getCachePath,
    touch(key) {
      const entry = getCachePath(key)
      const now = new Date()
      fs.utimesSync(entry, now, now)
    },
    // Retain downloads and partials for retries. A maintenance owner may reclaim
    // verified entries later; provisioning never performs recursive eviction.
    evict() {},
    async cleanPartials() {},
  }
}
