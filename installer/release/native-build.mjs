// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** One Cargo invocation owns the producing binaries, symbols and embedded compatibility declarations. */
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'
import { selectArtifacts } from './application-artifacts.mjs'
import { nativeConfiguration } from './native-configuration.mjs'
import { requireBootstrapImages } from './windows-imports.mjs'

export async function buildNative(component, profile, qualification, platform, inspect) {
  const names = component === 'application' ? ['sugarsubstitute', 'sugarsubstitute-launcher', 'sugarsubstitute-updater'] : component === 'updater' ? ['sugarsubstitute-updater'] : undefined
  if (!names || !['dev', 'release'].includes(profile) || (qualification !== undefined && qualification !== '--local-k1')) throw new Error('Invalid native build request')
  const target = process.env.CARGO_TARGET_DIR
  if (!target || !path.isAbsolute(target)) throw new Error('Set an explicit absolute CARGO_TARGET_DIR')
  const workspace = fileURLToPath(new URL('../../', import.meta.url))
  const configuration = await nativeConfiguration(target, platform, workspace, process.env)
  const args = ['build', '--locked', '-p', 'sugarsubstitute', ...names.flatMap(name => ['--bin', name]), '--target', configuration.target, '--profile', profile, '--message-format=json-render-diagnostics']
  if (qualification === '--local-k1') args.push('--features', 'sugarsubstitute-supervision/local-k1-symbol-trust,k1-native-qualification')
  const result = spawnSync('cargo', args, { cwd: workspace, env: configuration.environment, encoding: 'utf8', windowsHide: true, maxBuffer: 32 * 1024 ** 2 })
  if (result.stderr) process.stderr.write(result.stderr)
  if (result.error) throw result.error
  if (result.status !== 0) throw new Error(`Cargo build failed (${result.status})`)
  const messages = result.stdout.split(/\r?\n/).filter(Boolean).map(line => JSON.parse(line))
  const selection = selectArtifacts(messages, path.resolve(workspace), path.resolve(target), component)
  return { ...selection, configuration: configuration.configuration, bootstrap: await requireBootstrapImages(selection, inspect) }
}
