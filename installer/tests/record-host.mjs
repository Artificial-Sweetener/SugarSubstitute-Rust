// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** In-memory external storage boundary for worker schema and wire tests. Native custody is tested in Rust. */
import path from 'node:path'
export function recordHost(NativeRecords, selected) {
  const files = new Map(), directories = new Set(), operations = []
  let listing = [], cursor = 0, revision = 0, snapshot, upload
  const missing = () => { throw Object.assign(new Error('missing fixture'), { code: 'ENOENT' }) }
  const location = operation => path.join(selected, ...(operation.child ? [operation.child] : []), operation.leaf)
  function perform(operation) {
    operations.push(operation)
    switch (operation.op) {
      case 'open':
        if (!directories.has(selected)) { if (!operation.create) missing(); directories.add(selected) }
        return [{ path: selected }]
      case 'page':
        if (operation.restart) {
          listing = [...directories].filter(file => file !== selected && path.dirname(file) === selected)
            .map(file => ({ name: path.basename(file), directory: true }))
            .concat([...files.keys()].filter(file => path.dirname(file) === selected).map(file => ({ name: path.basename(file), directory: false })))
          cursor = 0
        }
        return [{ entries: listing.slice(cursor, cursor += 32) }]
      case 'create_child': {
        const file = path.join(selected, operation.name)
        if (directories.has(file)) throw Object.assign(new Error('exists'), { code: 'EEXIST' })
        directories.add(file); return [{ path: file }]
      }
      case 'read': {
        const file = location(operation), bytes = files.get(file)
        if (bytes === undefined) missing()
        snapshot = { file, bytes, revision: ++revision }
        const buffer = Buffer.from(bytes), frames = []
        for (let offset = 0; offset < buffer.length; offset += 8192) frames.push({ hex: buffer.subarray(offset, offset + 8192).toString('hex'), revision })
        return frames.length ? frames : [{ hex: '', revision }]
      }
      case 'begin':
        upload = { file: location(operation), original: operation.revision === null ? undefined : snapshot, bytes: [] }
        return [{}]
      case 'chunk': upload.bytes.push(Buffer.from(operation.hex, 'hex')); return [{}]
      case 'commit':
        if (!upload.original && files.has(upload.file)) throw Object.assign(new Error('exists'), { code: 'EEXIST' })
        files.set(upload.file, Buffer.concat(upload.bytes).toString('utf8')); upload = undefined; return [{}]
      default: throw new Error('Unexpected mock boundary operation')
    }
  }
  const client = new NativeRecords((id, data) => {
    let result
    try { result = perform(data.operation).map(data => ({ ok: true, data })) }
    catch (cause) { result = [{ ok: false, error: { code: cause.code ?? 'EIO', osCode: null } }] }
    result.forEach((item, index) => client.accept({ id, input: { serial: data.serial, index, done: index + 1 === result.length, result: item } }))
  })
  client.begin('fixture', selected)
  return { client, files, directories, operations }
}
