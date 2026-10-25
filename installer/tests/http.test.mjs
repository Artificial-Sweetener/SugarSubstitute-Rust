// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Exercise the actual patched upstream network owners under the shipped Node runtime. */
import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { build } from 'esbuild'
import { runtimeExecutable, runtimeFlags } from '../runtime.mjs'

test('standalone Node preserves upstream download, resume, cancellation and metadata behavior', { timeout: 45000 }, async () => {
  const destination = process.env.SUBSTITUTE_INSTALLER_BUILD
  const receipt = JSON.parse(await fs.readFile(path.join(destination, 'worker/build-receipt.json'), 'utf8'))
  const entry = receipt.inputs.find(value => value.includes('/src/main/lib/download.ts'))
  assert.ok(entry, 'Built upstream downloader must appear in the worker graph')
  const upstream = entry.slice(0, entry.indexOf('/src/main/'))
  const workspace = await fs.mkdtemp(path.join(destination, 'http-'))
  const output = path.join(workspace, 'network.cjs')
  const worker = fileURLToPath(new URL('../worker/', import.meta.url))
  const networking = path.resolve(worker, '../../crates/sugarsubstitute-networking/worker')
  await build({
    stdin: { contents: `export { download, downloadMetaPath } from ${JSON.stringify(path.resolve(entry))}; export { fetchJSON } from ${JSON.stringify(path.resolve(upstream, 'src/main/lib/fetch.ts'))}; export * from ${JSON.stringify(path.join(networking, 'proxy.ts'))}; export { createRequest } from ${JSON.stringify(path.join(networking, 'http.ts'))}; import { NativeMetadata, bindNativeMetadata, metadataRoot } from ${JSON.stringify(path.join(worker, 'metadata.ts'))}; import { createMetadataHost } from ${JSON.stringify(fileURLToPath(new URL('./native-metadata-host.mjs', import.meta.url)))}; import { bindDiagnostics } from ${JSON.stringify(path.join(worker, 'diagnostics.mjs'))}; export const diagnosticEvents = []; export async function initializeMetadata() { bindDiagnostics((level, message) => diagnosticEvents.push({ level, message })); const host = createMetadataHost(NativeMetadata, metadataRoot()); bindNativeMetadata(host.client); await host.client.load(); }`, resolveDir: process.cwd() },
    outfile: output, bundle: true, platform: 'node', target: 'node24', format: 'cjs',
    plugins: [{ name: 'actual-provisioning-adapters', setup(builder) {
      const aliases = new Map([['src/main/settings.ts', 'settings.ts'], ['src/main/lib/paths.ts', 'paths.ts'], ['src/main/lib/provisioningTransport.ts', '../../crates/sugarsubstitute-networking/worker/http.ts'], ['src/main/lib/native-metadata.ts', 'metadata.ts']])
      builder.onResolve({ filter: /^\./ }, args => {
        const relative = path.relative(upstream, path.resolve(args.resolveDir, args.path) + '.ts').replaceAll('\\', '/')
        if (aliases.has(relative)) return { path: path.join(worker, aliases.get(relative)) }
      })
    } }],
  })
  const env = { ...process.env }
  delete env.NODE_TEST_CONTEXT
  delete env.NODE_OPTIONS
  for (const name of Object.keys(env)) if (/^(?:https?_proxy|all_proxy|no_proxy)$/i.test(name)) delete env[name]
  const result = await promisify(execFile)(runtimeExecutable(destination), [...runtimeFlags, '--test-reporter=tap',
    fileURLToPath(new URL('./http-scenarios.mjs', import.meta.url)), output, '--setup-workspace', workspace, '--setup-cache-metadata', path.join(workspace, 'metadata'),
  ], { windowsHide: true, timeout: 35000, maxBuffer: 1024 * 1024, env })
  assert.match(result.stdout, /# fail 0/)
  await fs.writeFile(path.join(workspace, 'network-tests.tap'), result.stdout)
})
