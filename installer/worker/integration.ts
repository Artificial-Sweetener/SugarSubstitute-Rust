// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Complete SugarCubes dependencies in the selected runtime before desktop activation. */
import fs from 'node:fs/promises'
import path from 'node:path'
import { execFile } from 'node:child_process'
import { standalone } from '@upstream/sources/standalone/index'
import { getActiveUvPath } from '@upstream/lib/pythonEnv'
import { inspectProgram } from './dependency-inspection.mjs'
import { configureModelRoot } from './model-root.mjs'
import type { ProgressSink } from './product-messages'

type Runtime = { directory: string; python: string; frontend: string; metadata: string }
type Send = ProgressSink

/** Preserve failed package operations as failures; cancellation waits for the child exit callback. */
function run(command: string, args: string[], cwd: string, signal: AbortSignal): Promise<string> {
  signal.throwIfAborted()
  return new Promise((resolve, reject) => {
    execFile(command, args, {
      cwd, signal, windowsHide: true, maxBuffer: 4 * 1024 * 1024,
      env: { ...process.env, PYTHONDONTWRITEBYTECODE: '1' },
    }, (cause, stdout, stderr) => {
      if (cause) reject(new Error(`SugarCubes dependency operation failed: ${stderr.slice(-4096)}`, { cause }))
      else resolve(stdout)
    })
  })
}

/** Read an existing node's own requirements, or the authenticated bundled node when installing it. */
export async function integrate(input: Runtime & { models: string }, signal: AbortSignal, send: Send): Promise<Runtime> {
  const { models, ...runtime } = input
  const existing = path.join(runtime.directory, 'custom_nodes', 'SugarCubes', 'pyproject.toml')
  let project = existing
  try { await fs.access(existing) } catch (cause) {
    if ((cause as NodeJS.ErrnoException).code !== 'ENOENT') throw cause
    project = path.join(__dirname, '..', '..', '..', 'sugarcubes', 'pyproject.toml')
  }
  const inspect = async (): Promise<string[]> => {
    const result: unknown = JSON.parse(await run(runtime.python, ['-B', '-c', inspectProgram, project], runtime.directory, signal))
    if (!Array.isArray(result) || result.some(value => typeof value !== 'string')) {
      throw new Error('SugarCubes dependency check returned an invalid result')
    }
    return result
  }
  send('progress', { phase: 'integration', percent: 0, statusCode: 'checkingSugarCubes' })
  const missing = await inspect()
  if (missing.length) {
    send('progress', { phase: 'integration', percent: -1, statusCode: 'installingSugarCubes' })
    const installation = await standalone.probeInstallation(runtime.directory)
    const uv = installation ? getActiveUvPath(installation as Parameters<typeof getActiveUvPath>[0]) : undefined
    if (uv) {
      await run(uv, ['pip', 'install', '--python', runtime.python, ...missing], runtime.directory, signal)
    } else {
      await run(runtime.python, ['-m', 'pip', 'install', '--disable-pip-version-check', ...missing], runtime.directory, signal)
    }
    if ((await inspect()).length) throw new Error('SugarCubes dependencies did not pass verification')
  }
  signal.throwIfAborted()
  await run(runtime.python, ['-B', '-c', configureModelRoot, path.dirname(project), runtime.directory, models],
    runtime.directory, signal)
  send('progress', { phase: 'integration', percent: 100, statusCode: 'sugarCubesReady' })
  return runtime
}
