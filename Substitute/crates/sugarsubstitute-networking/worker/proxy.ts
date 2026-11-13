// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Correlate bounded, cancellable Windows proxy lookups with the authenticated Rust host. */
export type Route = { kind: 'direct' } | { kind: 'proxy'; url: string }
type Reply = { routes?: Route[]; error?: string }
type Pending = { finish: (cause?: Error, routes?: Route[]) => void }
type Sender = (id: string, event: string, data: unknown) => void
let sender: Sender | undefined
let lifetime = new AbortController()
let sequence = 0
const pending = new Map<string, Pending>()

/** Install once when the worker establishes its host transport. */
export function connectProxyHost(send: Sender): void {
  if (sender) throw new Error('Proxy host already connected')
  lifetime = new AbortController()
  sender = send
}

/** Downloads, including explicitly proxied ones, belong to the host transport lifetime. */
export function proxyHostSignal(): AbortSignal { return lifetime.signal }

/** A transport loss retires every outstanding lookup and rejects subsequent requests. */
export function closeProxyHost(): void {
  lifetime.abort(new Error('Proxy host disconnected'))
  sender = undefined
  for (const request of pending.values()) request.finish(new Error('Proxy host disconnected'))
}

/** Deliver only a currently owned reply; late cancellation replies cannot revive a request. */
export function receiveProxyResult(id: string, input: Reply): void {
  const request = pending.get(id)
  if (!request) return
  request.finish(input.error ? new Error(input.error) : undefined, input.routes)
}

/** Resolve each destination independently, retaining a finite host-response deadline. */
export function systemRoutes(address: string, signal: AbortSignal): Promise<Route[]> {
  signal.throwIfAborted()
  if (process.platform !== 'win32') return Promise.resolve([{ kind: 'direct' }])
  const send = sender
  if (!send) return Promise.reject(new Error('Windows proxy host is unavailable'))
  if (pending.size >= 8) return Promise.reject(new Error('Proxy resolution capacity reached'))
  const id = `proxy_${++sequence}`
  return new Promise((resolve, reject) => {
    const finish = (cause?: Error, routes?: Route[]) => {
      if (!pending.delete(id)) return
      clearTimeout(timer)
      signal.removeEventListener('abort', abort)
      if (cause) reject(cause)
      else if (!routes?.length) reject(new Error('Windows returned no proxy routing decision'))
      else resolve(routes)
    }
    const cancel = (cause: Error) => {
      finish(cause)
      try { send(id, 'proxy_cancel', {}) } catch { closeProxyHost() }
    }
    const abort = () => cancel(new Error('Proxy resolution cancelled', { cause: signal.reason }))
    const timer = setTimeout(() => cancel(new Error('Windows proxy resolution timed out')), 65_000)
    pending.set(id, { finish })
    signal.addEventListener('abort', abort, { once: true })
    try { send(id, 'proxy_resolve', { url: address }) } catch (cause) { finish(cause instanceof Error ? cause : new Error(String(cause))) }
  })
}
