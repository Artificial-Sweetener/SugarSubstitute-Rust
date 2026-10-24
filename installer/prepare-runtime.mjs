// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Fetch the pinned worker runtime into the explicit packaging artifact directory. */
import { createRequire } from 'node:module'
import fs from 'node:fs/promises'
import path from 'node:path'
const require = createRequire(import.meta.url)
const { downloadArtifact } = require('@electron/get')
const extract = require('extract-zip')
const { version } = require('electron/package.json')
const checksums = require('electron/checksums.json')
const destination = process.env.SUBSTITUTE_INSTALLER_BUILD
if (!destination || !path.isAbsolute(destination)) throw new Error('An absolute build directory is required')
const cacheRoot = path.join(destination, 'download-cache')
const runtime = path.join(destination, 'electron')
const archive = await downloadArtifact({
  version, artifactName: 'electron', platform: process.platform, arch: process.arch,
  cacheRoot, checksums,
})
await extract(archive, { dir: runtime })
await fs.cp(path.dirname(require.resolve('7zip-bin/package.json')),
  path.join(destination, 'worker/node_modules/7zip-bin'), { recursive: true })
console.log(`Prepared Electron ${version} at ${runtime}`)
