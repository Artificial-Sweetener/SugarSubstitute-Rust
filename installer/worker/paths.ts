// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Isolate upstream metadata and downloads inside the installer's explicit workspace. */
import path from 'node:path'
import { argument } from './arguments'

function root(): string {
  const value = argument('workspace')
  if (!value || !path.isAbsolute(value)) throw new Error('Provisioning workspace is required')
  return value
}

export function cacheDir(): string { return path.join(root(), 'cache-v1') }
export function configDir(): string { return path.join(root(), 'configuration') }
export function dataDir(): string { return path.join(root(), 'data') }
export function stateDir(): string { return path.join(root(), 'state') }
