// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Versioned, bounded host protocol. Selection values carry no download or execution authority. */
export const VERSION = 1
export const MAX_FRAME_BYTES = 65536
const fields = new Set(['release', 'comfyVersion', 'variant'])

function text(value, name, maximum = 4096) {
  if (typeof value !== 'string' || !value.length || value.includes('\0') ||
      Buffer.byteLength(value) > maximum) throw new Error(`Invalid ${name}`)
  return value
}

function keys(object, allowed) {
  if (!object || typeof object !== 'object' || Array.isArray(object) ||
      Object.keys(object).some(key => !allowed.includes(key))) throw new Error('Unknown protocol field')
}

export function parseCommand(bytes) {
  if (Buffer.byteLength(bytes) > MAX_FRAME_BYTES) throw new Error('Command exceeds protocol bound')
  const value = JSON.parse(bytes)
  keys(value, ['version', 'id', 'operation', 'input'])
  if (value.version !== VERSION) throw new Error('Unsupported provisioning protocol')
  text(value.id, 'request identity', 80)
  if (!/^[a-zA-Z0-9_-]+$/.test(value.id)) throw new Error('Invalid request identity')
  const input = value.input ?? {}
  switch (value.operation) {
    case 'metadata_reply':
    case 'storage_reply':
      keys(input, ['version', 'serial', 'index', 'done', 'result'])
      if (input.version !== 1 || !Number.isSafeInteger(input.serial) || input.serial <= 0 ||
          !Number.isSafeInteger(input.index) || input.index < 0 || input.index >= 8 || typeof input.done !== 'boolean') throw new Error('Invalid native storage reply')
      keys(input.result, ['ok', 'data', 'error'])
      if (input.result.ok === true) {
        if ('error' in input.result || !input.result.data || typeof input.result.data !== 'object' || Array.isArray(input.result.data)) throw new Error('Invalid native storage result')
      } else if (input.result.ok === false) {
        keys(input.result.error, ['code', 'osCode'])
        if ('data' in input.result || !['ENOENT', 'EEXIST', 'EACCES', 'EBUSY', 'EINTR', 'ETIMEDOUT', 'EIO', ...(value.operation === 'metadata_reply' ? ['ENOSPC'] : [])].includes(input.result.error.code) ||
            input.result.error.osCode !== null && !Number.isSafeInteger(input.result.error.osCode)) throw new Error('Invalid native storage error')
      } else throw new Error('Invalid native storage outcome')
      break
    case 'options':
      keys(input, ['field', 'selections', 'mirrors', 'locale'])
      if (!fields.has(input.field)) throw new Error('Unsupported provisioning field')
      selections(input.selections ?? {})
      if (input.mirrors !== undefined && typeof input.mirrors !== 'boolean') throw new Error('Invalid mirrors option')
      if (input.locale !== undefined) text(input.locale, 'locale', 16)
      break
    case 'inspect':
      keys(input, ['directory', 'python'])
      text(input.directory, 'directory')
      if (input.python !== undefined) text(input.python, 'python')
      break
    case 'provision':
      keys(input, ['directory', 'selectionToken'])
      text(input.directory, 'directory')
      text(input.selectionToken, 'selection token', 80)
      break
    case 'integrate':
      keys(input, ['directory', 'python', 'frontend', 'metadata', 'models'])
      for (const field of ['directory', 'python', 'frontend', 'metadata', 'models']) text(input[field], field)
      break
    case 'cancel':
      keys(input, ['requestId'])
      text(input.requestId, 'request identity', 80)
      break
    default: throw new Error('Unknown provisioning operation')
  }
  return { ...value, input }
}

function selections(value) {
  keys(value, [...fields])
  for (const [key, entry] of Object.entries(value)) text(entry, key, 128)
}

/** Keep framing bounded even when a hostile peer never sends a newline. */
export class Decoder {
  #bytes = Buffer.alloc(0)
  push(chunk) {
    const frames = []
    let start = 0
    for (let index = 0; index < chunk.length; index++) {
      if (chunk[index] !== 10) continue
      const part = chunk.subarray(start, index)
      if (this.#bytes.length + part.length > MAX_FRAME_BYTES) throw new Error('Command exceeds protocol bound')
      frames.push(parseCommand(Buffer.concat([this.#bytes, part])))
      this.#bytes = Buffer.alloc(0)
      start = index + 1
    }
    const tail = chunk.subarray(start)
    if (this.#bytes.length + tail.length > MAX_FRAME_BYTES) throw new Error('Command exceeds protocol bound')
    this.#bytes = Buffer.concat([this.#bytes, tail])
    return frames
  }
  end() {
    if (this.#bytes.length) throw new Error('Truncated provisioning command')
  }
}
