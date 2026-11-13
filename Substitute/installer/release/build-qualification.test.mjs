// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Local HTTP admission requires a deliberate qualification build, never an ambient override. */
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { nativeBuildArguments, qualificationFeatures } from './build-qualification.mjs'

/** Read the application's declared default features, which ordinary builds ship. */
function shippedFeatures() {
  const manifest = readFileSync(new URL('../../crates/sugarsubstitute/Cargo.toml', import.meta.url), 'utf8')
  const declared = /^default\s*=\s*\[([^\]]*)\]/m.exec(manifest)
  assert.ok(declared, 'The application manifest declares no default features')
  return [...declared[1].matchAll(/"([^"]+)"/g)].map(match => match[1])
}

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

test('ordinary installer builds ship the default feature set, including native composition', () => {
  const args = nativeBuildArguments(['sugarsubstitute'], 'x86_64-pc-windows-msvc', 'release', qualificationFeatures(undefined))
  assert.deepEqual(args.slice(0, 4), ['build', '--locked', '-p', 'sugarsubstitute'])
  assert.ok(!args.some(argument => argument.includes('feature')), 'ordinary builds must not select features')
  const shipped = shippedFeatures()
  assert.ok(shipped.includes('native-composition'))
  for (const qualification of ['k1-native-qualification', 'composition-qualification', 'local-k1-symbol-trust'])
    assert.ok(!shipped.some(feature => feature.endsWith(qualification)), `${qualification} must stay opt-in`)
})

test('qualification builds add capabilities on top of the shipped defaults', () => {
  const args = nativeBuildArguments(['sugarsubstitute'], 'x86_64-pc-windows-msvc', 'dev', qualificationFeatures('--local-k1'))
  assert.ok(!args.includes('--no-default-features'))
  assert.deepEqual(args.slice(-2), ['--features', 'sugarsubstitute-supervision/local-k1-symbol-trust,k1-native-qualification'])
})