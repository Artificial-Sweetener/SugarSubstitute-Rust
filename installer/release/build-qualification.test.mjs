// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Local HTTP admission requires a deliberate qualification build, never an ambient override. */
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { qualificationFeatures } from './build-qualification.mjs'

test('ordinary and crash qualification builds do not enable local update feeds', () => {
  assert.deepEqual(qualificationFeatures(undefined), [])
  assert.deepEqual(qualificationFeatures('--local-k1'), [
    'sugarsubstitute-supervision/local-k1-symbol-trust', 'k1-native-qualification'
  ])
})

test('local update qualification explicitly enables loopback with native qualification', () => {
  assert.deepEqual(qualificationFeatures('--local-updates'), [
    'sugarsubstitute-supervision/local-k1-symbol-trust', 'k1-native-qualification',
    'sugarsubstitute-supervision/local-update-qualification'
  ])
  for (const value of ['', '--features', '--local-update', null, true, {}]) {
    assert.throws(() => qualificationFeatures(value), /Unknown native qualification/)
  }
})
