// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Keep upstream cache semantics on the actual native grant without writable filesystem capabilities. */
import type { Cache } from '@upstream/lib/cache'
import { nativeDownloads } from './downloads'

/** Native storage owns admission, markers and cleanup after actual worker-family retirement. */
export function downloadCache(): Cache {
  const client = nativeDownloads()
  return {
    getCachePath: key => client.entry(key),
    length: file => client.length(file),
    async touch(key) { await client.entry(key) },
    async evict() { await client.validate() },
    async cleanPartials() { await client.validate() },
  }
}
