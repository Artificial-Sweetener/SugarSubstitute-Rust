// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Headless upstream worker. The Rust host owns its process family and visible window. */
import { app } from 'electron'
import fs from 'node:fs'
import { format } from 'node:util'
import { createConnection, type Socket } from 'node:net'
import { Decoder, VERSION } from './protocol.mjs'
import { configDir } from './paths'
import { CacheStorageError, observeCachePressure } from './cache-policy'
import { options } from './selection'
import { inspect, provision } from './provision'
import { integrate } from './integration'
import { observe } from './telemetry'
import { argument } from './arguments'
import { Events } from './events.mjs'
import { NativeRecords, NativeRecordError, bindNativeRecords } from './records'
import { NativeMetadata, MetadataError, bindNativeMetadata, metadataRoot } from './metadata'
import { ProductFailure } from './product-messages'

declare const UPSTREAM_REVISION: string
let transport: Socket | undefined
let sequence = 0
let active: { id: string; control: AbortController } | undefined
let closing = false

function write(id: string | null, event: string, data: unknown): void {
  const bytes = JSON.stringify({ version: VERSION, id, sequence: ++sequence, event, data }) + '\n'
  if (Buffer.byteLength(bytes) > 65536) throw new Error('Provisioning event exceeds protocol bound')
  if (!transport || transport.destroyed) return
  if (transport.writableLength > 1024 * 1024) {
    active?.control.abort()
    throw new Error('Host stopped consuming provisioning events')
  }
  transport.write(bytes)
}
observeCachePressure(namespace => write(null, 'cache_pressure', { namespace }))
const records = new NativeRecords((id, data) => write(id, 'storage', data))
bindNativeRecords(records)
const metadata = new NativeMetadata((id, data) => write(id, 'metadata', data), metadataRoot())
bindNativeMetadata(metadata)
const events = new Events(write)
const send = (id: string | null, event: string, data: unknown) => events.send(id, event, data)
const flushing = setInterval(() => events.flush(), 100)
app.on('will-quit', () => clearInterval(flushing))
// Electron's default uncaught-error handler opens a native dialog. This worker
// reports through its retained transport; its host owns the visible recovery UI.
function fatal(cause: unknown): void {
  closing = true
  active?.control.abort()
  try {
    send(null, 'fatal', { message: cause instanceof Error ? cause.message : String(cause) })
  } finally {
    app.exit(1)
  }
}
process.on('uncaughtException', fatal)
process.on('unhandledRejection', fatal)

for (const level of ['log', 'info', 'warn', 'error', 'debug'] as const) {
  console[level] = (...args: unknown[]) => {
    send(active?.id ?? null, 'diagnostic', { level, message: format(...args).slice(-8192) })
  }
}
fs.mkdirSync(configDir(), { recursive: true })
app.setPath('userData', configDir())
app.setPath('sessionData', argument('cache-runtime'))
app.commandLine.appendSwitch('disk-cache-size', String(64 * 1024 * 1024))
app.disableHardwareAcceleration()
observe((event, properties) => send(active?.id ?? null, 'diagnostic', { event, properties }))

async function dispatch(command: ReturnType<Decoder['push']>[number]): Promise<void> {
  if (command.operation === 'metadata_reply') { metadata.accept(command); return }
  if (command.operation === 'storage_reply') { records.accept(command); return }
  if (command.operation === 'cancel') {
    if (!active || command.input.requestId !== active.id) {
      send(command.id, 'failed', { message: 'Cancellation does not match the active request' })
      return
    }
    active.control.abort()
    send(command.id, 'completed', { cancellationRequested: true })
    return
  }
  if (active || closing) {
    send(command.id, 'failed', { message: 'Provisioning worker is busy or closing' })
    return
  }
  const control = new AbortController()
  active = { id: command.id, control }
  try {
    records.begin(command.id, command.input.directory)
    metadata.begin(command.id)
    await metadata.load()
    control.signal.throwIfAborted()
    let result: unknown
    if (command.operation === 'options') result = await options(command.input)
    else if (command.operation === 'inspect') result = await inspect(command.input, control.signal)
    else if (command.operation === 'integrate') result = await integrate(command.input, control.signal,
      (event, data) => send(command.id, event, data))
    else result = await provision(command.input, control.signal,
      (event, data) => send(command.id, event, data))
    await metadata.drain()
    control.signal.throwIfAborted()
    send(command.id, 'completed', result)
  } catch (cause) {
    send(command.id, control.signal.aborted ? 'cancelled' : 'failed', {
      message: cause instanceof Error ? cause.message : String(cause),
      ...(cause instanceof CacheStorageError || cause instanceof NativeRecordError || cause instanceof MetadataError || cause instanceof ProductFailure ? { statusCode: cause.statusCode } : {}),
    })
  } finally {
    await metadata.drain()
    metadata.close()
    records.close()
    active = undefined
    if (closing) app.quit()
  }
}

void app.whenReady().then(() => {
  const port = Number(argument('port'))
  const token = argument('token')
  if (!Number.isInteger(port) || port < 1 || port > 65535 || !token || !/^[a-f0-9]{64}$/.test(token)) {
    throw new Error('Authenticated provisioning transport is required')
  }
  const socket = createConnection({ host: '127.0.0.1', port })
  transport = socket
  const decoder = new Decoder()
  socket.on('connect', () => {
    socket.write(JSON.stringify({ version: VERSION, event: 'authenticate', token }) + '\n')
    send(null, 'ready', { cacheProtocol: 1, storageProtocol: 1, metadataProtocol: 1, textProtocol: 1, upstreamRevision: UPSTREAM_REVISION, operations: ['options', 'inspect', 'provision', 'integrate', 'cancel'] })
  })
  socket.on('data', (chunk: Buffer) => {
    try {
      for (const command of decoder.push(chunk)) void dispatch(command)
    } catch (cause) {
      closing = true
      const failure = cause instanceof Error ? cause : new Error(String(cause))
      metadata.close(failure)
      records.close(failure)
      active?.control.abort()
      send(null, 'protocol_error', { message: String(cause) })
      if (!active) socket.end(() => app.exit(2))
    }
  })
  socket.on('close', () => {
    closing = true
    metadata.close()
    records.close()
    active?.control.abort()
    if (!active) app.quit()
  })
  socket.on('error', cause => {
    metadata.close(cause)
    records.close(cause)
    console.error('Provisioning connection failed', cause.message)
    active?.control.abort()
    if (!active) app.exit(1)
  })
}).catch(cause => {
  send(null, 'failed', { message: String(cause) })
  app.exit(1)
})
