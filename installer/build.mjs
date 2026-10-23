// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import { build } from 'esbuild'
import fs from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'

const root = path.dirname(fileURLToPath(import.meta.url))
const source = path.resolve(root, '../third_party/comfy-desktop')
const destination = process.env.SUBSTITUTE_INSTALLER_BUILD
if (!destination || !path.isAbsolute(destination)) {
  throw new Error('Set SUBSTITUTE_INSTALLER_BUILD to an absolute artifact directory')
}
const output = path.resolve(destination)
if (output === source || source.startsWith(`${output}${path.sep}`)) {
  throw new Error('Build output must not contain the upstream checkout')
}
const git = (...args) => execFileSync('git', args, { encoding: 'utf8', windowsHide: true }).trim()
if (git('-C', source, 'status', '--porcelain')) throw new Error('Upstream checkout must be clean')
const requestedRevision = process.env.SUBSTITUTE_COMFY_REVISION ?? 'HEAD'
if (!/^[a-zA-Z0-9_/.~^-]+$/.test(requestedRevision) || requestedRevision.startsWith('-')) {
  throw new Error('Invalid upstream revision')
}
const revision = git('-C', source, 'rev-parse', '--verify', `${requestedRevision}^{commit}`)
await fs.mkdir(output, { recursive: true })
const upstream = await fs.mkdtemp(path.join(output, 'upstream-'))
const archive = path.join(upstream, 'source.tar')
git('-C', source, 'archive', '--format=tar', `--output=${archive}`, revision, 'src', 'lib', 'locales', 'package.json', 'LICENSE')
execFileSync('tar', ['-xf', archive, '-C', upstream], { windowsHide: true })
const upstreamPackage = JSON.parse(await fs.readFile(path.join(upstream, 'package.json'), 'utf8'))
const adapterPackage = JSON.parse(await fs.readFile(path.join(root, 'package.json'), 'utf8'))
if (upstreamPackage.devDependencies.electron !== adapterPackage.dependencies.electron) {
  throw new Error('Review and update the worker runtime pin for this upstream revision')
}
const patches = (await fs.readdir(path.join(root, 'patches'))).filter(p => p.endsWith('.patch')).sort()
const patchHashes = {}
for (const name of patches) {
  const patch = path.join(root, 'patches', name)
  git('-C', upstream, 'apply', '--check', patch)
  git('-C', upstream, 'apply', patch)
  patchHashes[name] = createHash('sha256').update(await fs.readFile(patch)).digest('hex')
}
const aliases = new Map([
  ['src/main/settings.ts', 'settings.ts'],
  ['src/main/lib/paths.ts', 'paths.ts'],
  ['src/main/lib/telemetry.ts', 'telemetry.ts'],
  ['src/main/lib/bundledScript.ts', 'bundled-script.ts'],
])
const bundle = path.join(output, 'worker/out/main/index.cjs')
const result = await build({
  entryPoints: [path.join(root, 'worker/main.ts')],
  outfile: bundle, bundle: true, platform: 'node', target: 'node22',
  format: 'cjs', metafile: true, sourcemap: true,
  external: ['electron', '7zip-bin'],
  nodePaths: [path.join(root, 'node_modules')],
  define: { UPSTREAM_REVISION: JSON.stringify(revision) },
  plugins: [{
    name: 'provisioning-boundaries',
    setup(builder) {
      builder.onResolve({ filter: /^@upstream\// }, args => ({
        path: path.join(upstream, 'src/main', args.path.slice('@upstream/'.length) + '.ts'),
      }))
      builder.onResolve({ filter: /^\./ }, args => {
        if (!args.importer.startsWith(upstream + path.sep)) return
        const resolved = path.resolve(args.resolveDir, args.path) + '.ts'
        const relative = path.relative(upstream, resolved).replaceAll('\\', '/')
        const adapter = aliases.get(relative)
        if (adapter) return { path: path.join(root, 'worker', adapter) }
      })
    },
  }],
})
if (result.warnings.length) throw new Error('Provisioning build emitted warnings')
await fs.cp(path.join(upstream, 'lib'), path.join(output, 'worker/lib'), { recursive: true })
await fs.cp(path.join(upstream, 'locales'), path.join(output, 'worker/locales'), { recursive: true })
await fs.writeFile(path.join(output, 'worker/package.json'), JSON.stringify({
  name: 'sugarsubstitute-provisioning-worker', version: '0.1.0',
  main: 'out/main/index.cjs', license: 'AGPL-3.0-or-later',
}))
await fs.copyFile(path.join(upstream, 'LICENSE'), path.join(output, 'worker/LICENSE'))
await fs.writeFile(path.join(output, 'worker/build-receipt.json'), JSON.stringify({
  schema_version: 1, upstream_revision: revision, patches: patchHashes,
  inputs: Object.keys(result.metafile.inputs).sort(),
}, null, 2))
console.log(`Built provisioning worker from ${revision} at ${bundle}`)
