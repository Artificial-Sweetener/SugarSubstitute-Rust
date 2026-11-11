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
/** Specify semantic matching and the common editable domain of related fields. */
import { expect, test } from '@jest/globals';
import {
  semanticFieldName,
  commonFieldDomain,
  acceptsFieldValue,
  choiceRestrictions,
} from '../../../frontend/comfyui/ui/relations/domain/FieldCompatibility.js';
import type { FieldContract } from '../../../frontend/comfyui/ui/relations/domain/RelationContracts.js';

/** Build a combo without depending on a live graph or renderer. */
function combo(id: string, values: string[], name = 'sampler_name'): FieldContract {
  return {
    id,
    nodeId: id,
    cubeId: id,
    label: id,
    name,
    semantic: semanticFieldName(name),
    value: values[0]!,
    domain: { kind: 'choice', values },
  };
}

test('normalizes known aliases without conflating independent seeds', () => {
  expect(semanticFieldName('Noise Seed')).toBe('seed');
  expect(semanticFieldName('sampler')).toBe('sampler_name');
  expect(semanticFieldName('variation_seed')).not.toBe('seed');
  expect(semanticFieldName('Tile Size')).toBe('tile_size');
});

test('keeps a superset sampler compatible and locks only its extra choices', () => {
  const fields = [
    combo('Standard', ['euler', 'heun']),
    combo('SimpleSyrup', ['custom1', 'heun', 'euler', 'custom2']),
  ];
  expect(commonFieldDomain(fields)).toEqual({ kind: 'choice', values: ['euler', 'heun'] });
  expect(choiceRestrictions(fields)).toEqual([
    { value: 'euler', blockers: [] },
    { value: 'heun', blockers: [] },
    { value: 'custom1', blockers: ['Standard'] },
    { value: 'custom2', blockers: ['Standard'] },
  ]);
  expect(acceptsFieldValue(commonFieldDomain(fields), 'custom1')).toBe(false);
});

test('rejects disjoint lists and mismatched semantics', () => {
  expect(commonFieldDomain([combo('a', ['euler']), combo('b', ['heun'])])).toBeNull();
  expect(
    commonFieldDomain([combo('a', ['normal']), combo('b', ['normal'], 'scheduler')]),
  ).toBeNull();
});

test('intersects numeric ranges and step grids without clamping members independently', () => {
  const first: FieldContract = {
    ...combo('a', []),
    name: 'cfg',
    semantic: 'cfg',
    value: 2,
    domain: { kind: 'number', minimum: 0, maximum: 20, step: 0.5, integer: false },
  };
  const second: FieldContract = {
    ...first,
    id: 'b',
    label: 'b',
    domain: { kind: 'number', minimum: 1, maximum: 10, step: 0.2, integer: false },
  };
  const domain = commonFieldDomain([first, second]);
  expect(domain).toEqual({ kind: 'number', minimum: 1, maximum: 10, step: 1, integer: false });
  expect(acceptsFieldValue(domain, 3)).toBe(true);
  expect(acceptsFieldValue(domain, 3.5)).toBe(false);
  expect(acceptsFieldValue(domain, 11)).toBe(false);
});

test('rejects incompatible numeric offsets and admits ordinary text and booleans', () => {
  const base: FieldContract = {
    ...combo('a', []),
    value: 0,
    domain: { kind: 'number', minimum: 0, maximum: 10, step: 2, integer: true },
  };
  expect(
    commonFieldDomain([
      base,
      { ...base, domain: { kind: 'number', minimum: 1, maximum: 9, step: 2, integer: true } },
    ]),
  ).toBeNull();
  expect(acceptsFieldValue({ kind: 'string' }, 'hello')).toBe(true);
  expect(acceptsFieldValue({ kind: 'boolean' }, 'true')).toBe(false);
});
