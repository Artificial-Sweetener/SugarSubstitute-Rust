// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Headless upstream worker. The Rust host owns its process family and visible window. */
import { diagnosticMessage, diagnostic, bindDiagnostics } from './diagnostics.mjs'
import { createConnection, type Socket } from 'node:net'
import { Decoder, VERSION, INTEGRATION_VERSION } from './protocol.mjs'
import { options } from './selection'
import { inspect, provision } from './provision'
import { integrate } from './integration'
import { observe } from './telemetry'
import { argument } from './arguments'
import { Events } from './events.mjs'
import { NativeRecords, NativeRecordError, bindNativeRecords } from './records'
import { NativeMetadata, MetadataError, bindNativeMetadata, metadataRoot } from './metadata'
import { NativeDownloads, DownloadStorageError, bindNativeDownloads, downloadsRoot } from './downloads'
import { ProductFailure } from './product-messages'
import { connectProxyHost, closeProxyHost, receiveProxyResult } from '../../crates/sugarsubstitute-networking/worker/proxy'

declare const UPSTREAM_REVISION: string
declare const WORKER_NODE_VERSION: string
if (process.versions.electron || process.versions.node !== WORKER_NODE_VERSION) throw new Error('The pinned standalone provisioning runtime is required')
let transport: Socket | undefined
let sequence = 0
let active: { id: string; control: AbortController } | undefined
let closing = false

function write(id: string | null, event: string, data: unknown): void {
  const version = event === 'ready' ? INTEGRATION_VERSION : VERSION
  const bytes = JSON.stringify({ version, id, sequence: ++sequence, event, data }) + '\n'
  if (Buffer.byteLength(bytes) > 65536) throw new Error('Provisioning event exceeds protocol bound')
  if (!transport || transport.destroyed) return
  if (transport.writableLength > 1024 * 1024) {
    active?.control.abort()
    throw new Error('Host stopped consuming provisioning events')
  }
  transport.write(bytes)
}
const records = new NativeRecords((id, data) => write(id, 'storage', data))
bindNativeRecords(records)
const metadata = new NativeMetadata((id, data) => write(id, 'metadata', data), metadataRoot())
bindNativeMetadata(metadata)
const downloads = new NativeDownloads((id, data) => write(id, 'downloads', data), downloadsRoot())
bindNativeDownloads(downloads)
const events = new Events(write)
const send = (id: string | null, event: string, data: unknown) => events.send(id, event, data)
bindDiagnostics((level: string, message: string) => send(active?.id ?? null, 'diagnostic', { level, message }))
const flushing = setInterval(() => events.flush(), 100)
flushing.unref()
/** Retire transport after the active operation observes cancellation and settles. */
function retire(code = process.exitCode ?? 0): void {
  closing = true
  process.exitCode = code
  clearInterval(flushing)
  closeProxyHost()
  events.flush()
  transport?.end()
}
function fatal(cause: unknown): void {
  closing = true
  active?.control.abort()
  try {
    send(null, 'fatal', { message: cause instanceof Error ? cause.message : String(cause) })
  } finally {
    retire(1)
  }
}
process.on('uncaughtException', fatal)
process.on('unhandledRejection', fatal)

for (const level of ['log', 'info', 'warn', 'error', 'debug'] as const) {
  console[level] = (...args: unknown[]) => {
    send(active?.id ?? null, 'diagnostic', { level, message: diagnosticMessage(args) })
  }
}
observe(event => send(active?.id ?? null, 'diagnostic', { message: diagnosticMessage([event]) }))

async function dispatch(command: ReturnType<Decoder['push']>[number]): Promise<void> {
  if (command.operation === 'proxy_result') { receiveProxyResult(command.id, command.input); return }
  if (command.operation === 'downloads_reply') { downloads.accept(command); return }
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
    downloads.begin(command.id)
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
    await downloads.drain()
    control.signal.throwIfAborted()
    send(command.id, 'completed', result)
  } catch (cause) {
    await downloads.drain()
    send(command.id, control.signal.aborted ? 'cancelled' : 'failed', {
      message: cause instanceof Error ? cause.message : String(cause),
      ...(cause instanceof NativeRecordError || cause instanceof MetadataError || cause instanceof DownloadStorageError || cause instanceof ProductFailure ? { statusCode: cause.statusCode } : {}),
    })
  } finally {
    await downloads.drain()
    downloads.close()
    await metadata.drain()
    metadata.close()
    records.close()
    active = undefined
    if (closing) retire()
  }
}

async function connect(): Promise<void> {
  const port = Number(argument('port'))
  const token = argument('token')
  if (!Number.isInteger(port) || port < 1 || port > 65535 || !token || !/^[a-f0-9]{64}$/.test(token)) {
    throw new Error('Authenticated provisioning transport is required')
  }
  const socket = createConnection({ host: '127.0.0.1', port })
  socket.setNoDelay(true)
  transport = socket
  const decoder = new Decoder()
  socket.on('connect', () => {
    connectProxyHost(write)
    socket.write(JSON.stringify({ version: VERSION, event: 'authenticate', token }) + '\n')
    send(null, 'ready', { cacheProtocol: 2, storageProtocol: 1, metadataProtocol: 1, downloadsProtocol: 1, textProtocol: 1, integrationProtocol: INTEGRATION_VERSION, upstreamRevision: UPSTREAM_REVISION, operations: ['options', 'inspect', 'provision', 'integrate', 'cancel'] })
  })
  socket.on('data', (chunk: Buffer) => {
    try {
      for (const command of decoder.push(chunk)) void dispatch(command)
    } catch (cause) {
      closing = true
      const failure = cause instanceof Error ? cause : new Error(String(cause))
      downloads.close(failure)
      metadata.close(failure)
      records.close(failure)
      active?.control.abort()
      send(null, 'protocol_error', { message: String(cause) })
      process.exitCode = 2
      if (!active) retire(2)
    }
  })
  socket.on('close', () => {
    closeProxyHost()
    downloads.close()
    closing = true
    metadata.close()
    records.close()
    active?.control.abort()
    if (!active) retire()
  })
  socket.on('error', cause => {
    closeProxyHost()
    downloads.close(cause)
    metadata.close(cause)
    records.close(cause)
    diagnostic('error', 'Provisioning connection failed', cause.message)
    active?.control.abort()
    process.exitCode = 1
    if (!active) retire(1)
  })
}
void connect().catch(cause => {
  send(null, 'failed', { message: String(cause) })
  retire(1)
})
