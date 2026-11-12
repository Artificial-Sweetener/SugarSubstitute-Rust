// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { inspectProgram } from '../worker/dependency-inspection.mjs'

test('selected Python verifies missing, mismatched and satisfied dependencies without installation', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'substitute-dependencies-'))
  const project = path.join(root, 'pyproject.toml')
  const invoke = () => promisify(execFile)(process.env.SUBSTITUTE_TEST_PYTHON ?? 'python',
    ['-B', '-c', inspectProgram, project], {
      windowsHide: true, env: { ...process.env, PYTHONPATH: root, PYTHONDONTWRITEBYTECODE: '1' },
    })
  const write = dependencies => fs.writeFile(project,
    `[project]\nname="SugarCubes"\nrequires-python=">=3.12"\ndependencies=${JSON.stringify(dependencies)}\n`)
  await write(['substitute-qa-dependency==2.0'])
  assert.deepEqual(JSON.parse((await invoke()).stdout), ['substitute-qa-dependency==2.0'])
  const metadata = path.join(root, 'substitute_qa_dependency-1.0.dist-info')
  await fs.mkdir(metadata)
  await fs.writeFile(path.join(metadata, 'METADATA'), 'Name: substitute-qa-dependency\nVersion: 1.0\n')
  assert.deepEqual(JSON.parse((await invoke()).stdout), ['substitute-qa-dependency==2.0'])
  await write(['substitute-qa-dependency==1.0', 'missing; python_version < "1.0"'])
  assert.deepEqual(JSON.parse((await invoke()).stdout), [])
  await write(['package @ https://example.invalid/package.whl'])
  await assert.rejects(invoke(), /registry requirements/)
  await fs.writeFile(project, '[project]\nname="SugarCubes"\nrequires-python=">=99"\n')
  await assert.rejects(invoke(), /different Python version/)
})
