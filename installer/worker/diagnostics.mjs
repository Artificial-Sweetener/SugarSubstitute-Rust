// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Project untrusted upstream diagnostics before retaining them in the bounded event queue. */
import contract from '../../crates/sugarsubstitute-contracts/src/diagnostics/current.json' with { type: 'json' }
let receiver
/** Bind the sole worker diagnostic lane before upstream provisioning is loaded. */
export function bindDiagnostics(sink) {
  if (receiver) throw new Error('Diagnostic routing is already bound')
  receiver = sink
}
/** Route authored worker observations through the same bounded projection as upstream console output. */
export function diagnostic(level, ...args) {
  if (!receiver) throw new Error('Diagnostic routing is unavailable')
  receiver(level, diagnosticMessage(args))
}
export function diagnosticText(value) {
  if (typeof value !== 'string') return '[untrusted diagnostic]'
  if (value.length > contract.worker_line_bytes || Buffer.byteLength(value) > contract.worker_line_bytes) return '[oversized diagnostic]'
  const clean = value.replace(/\p{Cc}/gu, ' ')
  const lower = clean.toLowerCase()
  if (contract.sensitive_text.some(term => lower.includes(term))) return '[redacted]'
  return clean
}

/** Avoid object inspection, getters, custom formatters and arbitrarily large concatenation. */
export function diagnosticMessage(args) {
  const parts = []
  let length = 0
  for (const value of args.slice(0, contract.worker_lines)) {
    const text = diagnosticText(typeof value === 'number' || typeof value === 'boolean' ? String(value) : value)
    const size = Buffer.byteLength(text) + (parts.length ? 1 : 0)
    if (length + size > contract.worker_line_bytes - 10) { parts.push('[omitted]'); break }
    parts.push(text)
    length += size
  }
  return parts.join(' ')
}
