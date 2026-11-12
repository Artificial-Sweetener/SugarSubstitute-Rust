// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Pinned standalone provisioning runtime and its explicit native launch arguments. */
import fs from 'node:fs'
import path from 'node:path'

export const runtime = JSON.parse(fs.readFileSync(new URL('./runtime.json', import.meta.url), 'utf8'))
if (runtime.schema_version !== 1 || !/^24\.\d+\.\d+$/.test(runtime.version)) throw new Error('Invalid worker runtime pin')
export const runtimeFlags = ['--use-system-ca', '--use-env-proxy', '--no-global-search-paths']

export function runtimeArchive(platform = process.platform, architecture = process.arch) {
  const value = runtime.archives[`${platform}-${architecture}`]
  if (!value || !/^[a-f0-9]{64}$/.test(value.sha256) || !/^node-v[\d.]+-[a-z0-9-]+\.(zip|tar\.(gz|xz))$/.test(value.file) || !value.file.startsWith(`node-v${runtime.version}-`)) {
    throw new Error('Unsupported or invalid provisioning runtime archive')
  }
  return { ...value, url: `https://nodejs.org/dist/v${runtime.version}/${value.file}` }
}

export function runtimeExecutable(build, platform = process.platform) {
  if (!path.isAbsolute(build)) throw new Error('Explicit worker build required')
  return path.join(build, 'worker', 'runtime', platform === 'win32' ? 'node.exe' : 'node')
}

export function workerArguments(build, args) {
  return [...runtimeFlags, path.join(build, 'worker', 'out', 'main', 'index.cjs'), ...args]
}
