// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Inject native cache custody into upstream's downloader; authored destinations stay upstream-owned. */
import { download as upstreamDownload, downloadMetaPath, type DownloadOptions, type DownloadStorage } from '@upstream/lib/download'
import { nativeDownloads } from './downloads'
import { NativeDownloadStream } from './download-stream'

/** Each opaque destination contains only this payload and its existing resume-sidecar name. */
export const download: typeof upstreamDownload = (url, destination, progress, options) => {
  const client = nativeDownloads()
  if (!client.contains(destination)) return upstreamDownload(url, destination, progress, options)
  const meta = downloadMetaPath(destination)
  const storage: DownloadStorage = {
    length: () => client.length(destination),
    metaLength: () => client.length(meta),
    readMeta: () => client.readFile(meta),
    writeMeta: data => client.writeFile(meta, data),
    remove: () => client.remove(destination),
    removeMeta: () => client.remove(meta),
    open: (append, expected, beforeOpen, prefix) => new NativeDownloadStream(client, destination, append, expected, beforeOpen, prefix),
  }
  const selected: DownloadOptions = typeof options === 'number' ? { _maxRedirects: options } : (options ?? {})
  return upstreamDownload(url, destination, progress, { ...selected, storage })
}
