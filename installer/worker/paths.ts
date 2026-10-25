// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Keep authoritative configuration separate from native disposable cache grants. */
import path from 'node:path'
import { argument } from './arguments'
import { metadataRoot } from './metadata'

function root(): string {
  const value = argument('workspace')
  if (!value || !path.isAbsolute(value)) throw new Error('Provisioning workspace is required')
  return value
}

export function cacheDir(): string { return metadataRoot() }
export function configDir(): string { return path.join(root(), 'configuration') }
export function dataDir(): string { return path.join(root(), 'data') }
export function stateDir(): string { return path.join(root(), 'state') }
