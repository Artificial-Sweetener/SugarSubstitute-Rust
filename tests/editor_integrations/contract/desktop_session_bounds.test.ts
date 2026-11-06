/** @jest-environment node */
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
/** Reject every shared shape bound and unknown member before semantic admission. */
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import * as protocol from '../../../integrations/shared/SessionValidation.generated.js';
import * as output from '../../../integrations/shared/SessionOutputValidation.generated.js';
import * as view from '../../../integrations/shared/SessionViewValidation.generated.js';
import * as snapshot from '../../../integrations/shared/SessionSnapshotValidation.generated.js';

interface Probe {
  readonly field: string;
  readonly kind: string;
  readonly value: unknown;
  readonly item?: unknown;
  readonly valid: boolean;
}
interface Shape {
  readonly name: string;
  readonly base: Record<string, unknown>;
  readonly probes: readonly Probe[];
}
const vectors = JSON.parse(
  readFileSync('integrations/contracts/session_shape_vectors.json', 'utf8'),
) as { readonly schemaSha256: string; readonly shapes: readonly Shape[] };
const shapes = vectors.shapes;
const guards: Readonly<Record<string, (value: unknown) => boolean>> = {
  ...protocol,
  ...output,
  ...view,
  ...snapshot,
};

test.each(shapes)('$name rejects every field bound and unknown or missing member', (shape) => {
  const guard = guards[`is${shape.name}`];
  if (!guard) throw new Error(`Missing generated guard for ${shape.name}`);
  expect(guard(shape.base)).toBe(true);
  for (const probe of shape.probes) {
    const value: Record<string, unknown> =
      probe.kind === 'missing'
        ? Object.fromEntries(Object.entries(shape.base).filter(([key]) => key !== probe.field))
        : { ...shape.base };
    if (probe.kind === 'string') value[probe.field] = 'x'.repeat(Number(probe.value));
    else if (probe.kind === 'array')
      value[probe.field] = Array.from({ length: Number(probe.value) }, () => probe.item);
    else if (probe.kind !== 'missing') value[probe.field] = probe.value;
    expect({ name: shape.name, field: probe.field, kind: probe.kind, valid: guard(value) }).toEqual(
      { name: shape.name, field: probe.field, kind: probe.kind, valid: probe.valid },
    );
  }
});

test('retains the complete shared shape and rejection inventory', () => {
  expect(
    createHash('sha256')
      .update(
        readFileSync('integrations/contracts/session.schema.json', 'utf8').replaceAll('\r\n', '\n'),
      )
      .digest('hex'),
  ).toBe(vectors.schemaSha256);
  expect(shapes).toHaveLength(37);
  expect(shapes.reduce((sum, shape) => sum + shape.probes.length, 0)).toBe(730);
});
