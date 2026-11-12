// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Fail closed when an upstream change adds desktop runtime or unbundled executable code. */
import { isBuiltin } from 'node:module'

export function requireHeadlessGraph(metafile) {
  for (const name of Object.keys(metafile.inputs)) {
    if (/(^|[/\\])(?:electron|@electron)(?:[/\\]|$)/i.test(name)) throw new Error('Electron cannot enter the provisioning worker')
  }
  for (const output of Object.values(metafile.outputs)) {
    for (const dependency of output.imports) {
      if (dependency.external && dependency.path !== '7zip-bin' && !isBuiltin(dependency.path)) {
        throw new Error(`Unapproved provisioning runtime import: ${dependency.path}`)
      }
    }
  }
}

/** Current producers ship one standalone runtime; historical readers live in the native engine. */
export function requireWorkerPayload(names) {
  for (const role of ['worker/package.json', 'worker/out/main/index.cjs', 'worker/runtime/node.exe', 'worker/runtime/LICENSE']) {
    if (!names.includes(role)) throw new Error(`Provisioning payload missing: ${role}`)
  }
  for (const name of names) {
    if (/(^|\/)(?:electron(?:\.exe)?|@electron)(?:\/|$)/i.test(name) ||
        (name.toLowerCase().startsWith('worker/runtime/') && !['worker/runtime/node.exe', 'worker/runtime/LICENSE'].includes(name))) {
      throw new Error(`Unapproved provisioning runtime payload: ${name}`)
    }
  }
}
