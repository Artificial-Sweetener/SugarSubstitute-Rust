// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Resolve only scripts shipped with this pinned provisioning worker. */
import path from 'node:path'
export function getBundledScriptPath(name: string): string {
  if (path.basename(name) !== name) throw new Error('Invalid bundled script name')
  return path.join(__dirname, '..', '..', 'lib', name)
}
