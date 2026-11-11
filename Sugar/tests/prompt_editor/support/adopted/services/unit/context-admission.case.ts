//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Canonical feature admission precedes scheduling adaptation and preserves actual valid context output. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  defaultPreferences,
  resolveFeatureProfile,
  validateFeatureContext,
  schedulingFeatureContext,
  captureFeatureContext,
  sameFeatureContext,
  type FeatureContext,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';

test('all malformed scheduling flags reject through the same feature owner before either workflow or library adaptation', () => {
  for (const mode of ['workflow', 'library'] as const)
    for (const loraScheduling of [undefined, null, '', 'true', 0, 1, {}, []]) {
      const context = { mode, loraScheduling } as unknown as FeatureContext;
      assert.throws(() => validateFeatureContext(context), TypeError);
      assert.throws(() => resolveFeatureProfile(defaultPreferences, context), TypeError);
      assert.throws(() => schedulingFeatureContext(context, null, null), TypeError);
    }
});

test('captured context retains original declarations across host mutation and compares literal arrays before policy resolution', () => {
  const entries: unknown[] = ['emphasis', 42],
    missing: import('../../../../../../frontend/sugarprompt/services/src/index.ts').FeatureId[] = [
      'lora_picker',
    ];
  const original = {
    mode: 'library' as const,
    field: { prompt_features: entries },
    unavailableFeatures: missing,
  };
  const captured = captureFeatureContext(original);
  assert.equal(sameFeatureContext(captured, captureFeatureContext(original)), true);
  const prepared = resolveFeatureProfile(defaultPreferences, captured);
  entries.push('lora_syntax');
  missing.length = 0;
  assert.equal(sameFeatureContext(captured, captureFeatureContext(original)), false);
  assert.deepEqual(captured.field!.prompt_features, ['emphasis', 42]);
  assert.deepEqual(captured.unavailableFeatures, ['lora_picker']);
  assert.deepEqual(prepared.rendererSyntax, ['emphasis']);
  assert.equal(Object.isFrozen(captured.field!.prompt_features), true);
  for (const invalid of [null, false, 0, '', [], { mode: 'library', field: null }])
    assert.throws(() => captureFeatureContext(invalid as unknown as FeatureContext), TypeError);
});

test('supplied context and field envelopes reject while unknown legacy metadata entries remain policy inputs', () => {
  for (const value of [
    null,
    false,
    0,
    '',
    [],
    {},
    { mode: 'library', extra: true },
    ...[undefined, null, false, 0, '', []].map((field) => ({
      mode: 'library',
      field,
    })),
  ])
    assert.throws(() => validateFeatureContext(value as unknown as FeatureContext), TypeError);
  const missing = ['emphasis', 'lora_picker'];
  // eslint-disable-next-line @typescript-eslint/no-array-delete -- Deliberately supply a sparse array to prove fail-closed input validation.
  delete missing[0];
  assert.throws(
    () =>
      validateFeatureContext({
        mode: 'library',
        unavailableFeatures: missing,
      } as FeatureContext),
    TypeError,
  );
  const library = Object.freeze({
    mode: 'library' as const,
    field: { prompt_features: ['emphasis', 'unknown', 42] },
  });
  assert.strictEqual(schedulingFeatureContext(library, null, null), library);
  const profile = resolveFeatureProfile(defaultPreferences, library);
  assert.deepEqual(profile.rendererSyntax, ['emphasis']);
});

test('valid workflow scheduling flags retain canonical policy results and equivalent prepared profile identity', () => {
  for (const loraScheduling of [true, false]) {
    const context = { mode: 'workflow' as const, loraScheduling };
    validateFeatureContext(context);
    const adapted = schedulingFeatureContext(context, null, null);
    assert.equal(adapted.loraScheduling, false);
    const profile = resolveFeatureProfile(defaultPreferences, adapted);
    assert.equal(
      profile.decisions.find((decision) => decision.feature === 'lora_picker')!.enabled,
      false,
    );
  }
});
