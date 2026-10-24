// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Invoke upstream install and post-install with an owned destination and explicit progress. */
import fs from 'node:fs/promises'
import path from 'node:path'
import { execFile } from 'node:child_process'
import { standalone } from '@upstream/sources/standalone/index'
import { download } from '@upstream/lib/download'
import { extract } from '@upstream/lib/extract'
import { downloadCache } from './cache'
import { getActivePythonPath } from '@upstream/lib/pythonEnv'
import { cacheGrants } from './cache-policy'
import { consumePlan } from './selection'
import { reserve, record, ready } from './ownership'
import { ProductFailure, type ProgressSink } from './product-messages'

type Send = ProgressSink

export async function provision(
  input: { directory: string; selectionToken: string }, signal: AbortSignal, send: Send
): Promise<unknown> {
  const plan = consumePlan(input.selectionToken)
  signal.throwIfAborted()
  const reusable = await ready(input.directory, plan)
  if (reusable) {
    try {
      return await verifyManaged(reusable.installation, reusable.directory, signal, send)
    } catch (cause) {
      signal.throwIfAborted()
      console.warn('Preserving an environment that did not pass verification; provisioning a fresh attempt', cause)
    }
  }
  const reservation = await reserve(input.directory)
  let installation = {
    ...plan, id: reservation.owner, sourceId: 'standalone',
    createdAt: new Date().toISOString(),
    name: 'SugarSubstitute ComfyUI', installPath: reservation.directory,
  }
  const sendProgress = (phase: string, data: { percent: number; status: string }) => {
    send('progress', { phase, ...data, textOwner: 'comfy-desktop' })
  }
  try {
    await standalone.install!(installation, {
      signal, sendProgress, download, extract, cache: downloadCache(cacheGrants().downloads),
    })
    await standalone.postInstall!(installation, {
      signal, sendProgress,
      update: async (data: Record<string, unknown>) => {
        installation = { ...installation, ...data }
        await record(reservation, 'provisioning', installation)
      },
    })
    signal.throwIfAborted()
    const environment = await verifyManaged(installation, reservation.directory, signal, send)
    await record(reservation, 'ready', installation)
    return environment
  } catch (cause) {
    try {
      await record(reservation, signal.aborted ? 'cancelled' : 'failed', installation)
    } catch (recordCause) {
      throw new AggregateError([cause, recordCause], 'Provisioning failed and recovery state could not be saved')
    }
    throw cause
  }
}

export async function inspect(
  input: { directory: string; python?: string }, signal: AbortSignal
): Promise<unknown> {
  const managed = await ready(input.directory)
  const directory = await fs.realpath(managed?.directory ?? input.directory)
  const upstream = await standalone.probeInstallation(directory)
  const comfy = upstream ? path.join(String(upstream.installPath), 'ComfyUI') : directory
  await fs.access(path.join(comfy, 'main.py'))
  const detected = upstream ? getActivePythonPath(upstream as Parameters<typeof getActivePythonPath>[0]) : null
  const candidates = input.python ? [input.python] : [
    ...(detected ? [detected] : []),
    path.join(comfy, '.venv', process.platform === 'win32' ? 'Scripts/python.exe' : 'bin/python3'),
    path.join(comfy, 'venv', process.platform === 'win32' ? 'Scripts/python.exe' : 'bin/python3'),
    path.join(comfy, '..', 'python_embeded', 'python.exe'),
  ]
  for (const candidate of candidates) {
    try { await fs.access(candidate) } catch { continue }
    const python = await fs.realpath(candidate)
    const environment = await checkPython(python, comfy, signal)
    return { directory: comfy, python, ...environment }
  }
  throw new ProductFailure('selectPython', 'Select the Python executable used by this ComfyUI installation')
}

/** Revalidate the actual runtime before reusing a completed provisioning attempt. */
async function verifyManaged(
  installation: Record<string, unknown>, directory: string, signal: AbortSignal, send: Send
): Promise<unknown> {
  const python = getActivePythonPath(installation as Parameters<typeof getActivePythonPath>[0])
  if (!python) throw new Error('Provisioning did not produce a Python environment')
  const relative = path.relative(directory, python)
  const actual = await fs.realpath(python)
  const equal = process.platform === 'win32' ? actual.toLowerCase() === path.resolve(python).toLowerCase() : actual === path.resolve(python)
  if (path.isAbsolute(relative) || relative.split(path.sep).includes('..') || !equal) {
    throw new Error('Managed Python must remain inside its owned environment')
  }
  const comfy = path.join(directory, 'ComfyUI')
  await fs.access(path.join(comfy, 'main.py'))
  send('progress', { phase: 'verify', percent: -1, statusCode: 'checkingPython' })
  const environment = await checkPython(python, comfy, signal)
  return { directory: comfy, python, ...environment }
}

/** Cold Torch imports remain cancellable without a wall-clock installation cutoff. */
function checkPython(python: string, cwd: string, signal: AbortSignal): Promise<{ frontend: string; metadata: string }> {
  const program = [
    'import json, pathlib, importlib.metadata',
    'import torch, aiohttp, comfyui_frontend_package',
    'distribution = importlib.metadata.distribution("comfyui_frontend_package")',
    'metadata = next(file for file in distribution.files if str(file).endswith(".dist-info/METADATA"))',
    'print(json.dumps({"frontend": str(pathlib.Path(comfyui_frontend_package.__file__).resolve().parent), "metadata": str(pathlib.Path(distribution.locate_file(metadata)).resolve())}))',
  ].join('\n')
  return new Promise((resolve, reject) => {
    execFile(python, ['-B', '-c', program], {
      cwd, signal, windowsHide: true, maxBuffer: 65536,
      env: { ...process.env, PYTHONDONTWRITEBYTECODE: '1' },
    }, (error, stdout, stderr) => {
      if (error) reject(new Error(`Python environment check failed: ${stderr.slice(-4096)}`, { cause: error }))
      else {
        try {
          const value = JSON.parse(stdout.trim().split(/\r?\n/).at(-1) ?? '')
          if (typeof value.frontend !== 'string' || typeof value.metadata !== 'string' ||
              !path.isAbsolute(value.frontend) || !path.isAbsolute(value.metadata)) {
            throw new Error('Python did not report an installed frontend distribution')
          }
          resolve(value)
        } catch (cause) { reject(cause) }
      }
    })
  })
}
