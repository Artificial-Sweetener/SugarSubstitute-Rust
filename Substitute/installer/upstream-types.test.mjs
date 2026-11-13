// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { declarations } from './upstream-types.mjs'

test('upstream type boundaries retain changed fields and referenced type imports verbatim', () => {
  const source = "import { app } from 'electron'\nimport type { Version } from './version'\nimport type { Unrelated } from './desktop'\nexport interface Installation { version: Version; newRequiredField: string }\nexport const window = app.open()\n"
  const result = declarations(source, ['Installation'])
  assert.match(result, /newRequiredField: string/)
  assert.match(result, /import type \{ Version \}/)
  assert.doesNotMatch(result, /electron|Unrelated|app\.open/)
  assert.throws(() => declarations(source, ['Removed']), /changed/)
  assert.throws(() => declarations('export interface {', ['Installation']), /parse/)
  const dependent = declarations('type Identity = { id: string }; type Nested = Identity & { flags: number }; export type Grant = Nested | null; const app = 1;', ['Grant'])
  assert.match(dependent, /type Identity/)
  assert.match(dependent, /type Nested/)
  assert.doesNotMatch(dependent, /const app/)
})
