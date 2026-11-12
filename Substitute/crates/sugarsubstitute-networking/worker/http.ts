// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** GET transport adapter; upstream owns redirects, download resumption, mirrors and progress. */
import http, { type ClientRequest, type IncomingMessage, type OutgoingHttpHeaders } from 'node:http'
import https from 'node:https'
import { EventEmitter } from 'node:events'
import { systemRoutes, proxyHostSignal, type Route } from './proxy'

const retryable = new Set(['ECONNREFUSED', 'ECONNRESET', 'EHOSTUNREACH', 'ENETUNREACH', 'ETIMEDOUT', 'ENOTFOUND', 'EAI_AGAIN'])

/** Node 24 owns HTTP forwarding, HTTPS CONNECT, certificates and environment bypass rules. */
function agent(url: URL, route: Route | undefined): http.Agent {
  const proxyEnv = route === undefined ? process.env : route.kind === 'direct' ? {} : {
    HTTP_PROXY: route.url, HTTPS_PROXY: route.url,
    NO_PROXY: process.env.no_proxy ?? process.env.NO_PROXY,
  }
  const options: http.AgentOptions & { proxyEnv: typeof proxyEnv } = { keepAlive: false, proxyEnv }
  return url.protocol === 'https:' ? new https.Agent(options) : new http.Agent(options)
}

/** Preserve upstream's synchronous request construction while proxy discovery is asynchronous. */
class ProvisioningRequest extends EventEmitter<{ response: [IncomingMessage]; error: [Error] }> {
  private readonly control = new AbortController()
  private readonly signal = AbortSignal.any([this.control.signal, proxyHostSignal()])
  private readonly headers: OutgoingHttpHeaders = {}
  private request?: ClientRequest
  private connection?: http.Agent
  private response?: IncomingMessage
  private ended = false
  private failed = false
  private idle = 0
  private timeout?: () => void

  constructor(private readonly url: URL) { super() }

  setHeader(name: string, value: string | number | readonly string[]): this {
    if (this.ended) throw new Error('Provisioning request headers already sent')
    http.validateHeaderName(name)
    for (const item of Array.isArray(value) ? value : [value]) http.validateHeaderValue(name, String(item))
    this.headers[name] = typeof value === 'object' ? [...value] : value
    return this
  }

  setTimeout(milliseconds: number, callback: () => void): this {
    this.idle = milliseconds
    this.timeout = callback
    this.request?.setTimeout(milliseconds, callback)
    return this
  }

  end(): this {
    if (this.ended) throw new Error('Provisioning request already sent')
    this.ended = true
    void this.start().catch(cause => this.fail(cause instanceof Error ? cause : new Error(String(cause))))
    return this
  }

  abort(): void { this.destroy() }

  destroy(cause?: Error): this {
    this.control.abort(cause)
    this.response?.destroy(cause)
    this.request?.destroy()
    this.connection?.destroy()
    if (cause) queueMicrotask(() => this.fail(cause))
    return this
  }

  private fail(cause: Error): void {
    if (this.failed) return
    this.failed = true
    this.emit('error', cause)
  }

  private async start(): Promise<void> {
    const signal = this.signal
    signal.throwIfAborted()
    const configured = this.url.protocol === 'https:'
      ? process.env.https_proxy ?? process.env.HTTPS_PROXY : process.env.http_proxy ?? process.env.HTTP_PROXY
    const routes: (Route | undefined)[] = configured !== undefined ? [undefined] : await systemRoutes(this.url.href, signal)
    for (let index = 0; index < routes.length; index++) {
      signal.throwIfAborted()
      try {
        this.response = await this.attempt(routes[index])
        if (signal.aborted) { this.response.destroy(); signal.throwIfAborted() }
        this.emit('response', this.response)
        return
      } catch (cause) {
        this.connection?.destroy()
        const code = (cause as NodeJS.ErrnoException).code
        if (signal.aborted || index + 1 === routes.length || !code || !retryable.has(code)) throw cause
      }
    }
  }

  private attempt(route: Route | undefined): Promise<IncomingMessage> {
    const connection = agent(this.url, route)
    this.connection = connection
    return new Promise((resolve, reject) => {
      const request = (this.url.protocol === 'https:' ? https : http).request(this.url, {
        headers: this.headers, agent: connection, signal: this.signal,
      })
      this.request = request
      if (this.idle) request.setTimeout(this.idle, this.timeout)
      request.once('error', reject)
      request.once('response', response => {
        response.once('close', () => connection.destroy())
        resolve(response)
      })
      request.end()
    })
  }
}

export function createRequest(address: string): ProvisioningRequest {
  const url = new URL(address)
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password || url.hash) throw new Error('Unsupported provisioning transport')
  return new ProvisioningRequest(url)
}
