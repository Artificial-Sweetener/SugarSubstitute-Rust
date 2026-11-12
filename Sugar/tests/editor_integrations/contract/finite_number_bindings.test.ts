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
/** Keep full finite IEEE bounds equivalent and compact in generated Rust validators. */
import { rustProperty } from '../../../scripts/integrations/RustContractProperty.js';

const definitions = { names: new Set<string>(), enums: new Set<string>() };

test('emits a finite predicate only for the inclusive full representable range', () => {
  const property = rustProperty(
    { type: 'number', minimum: -Number.MAX_VALUE, maximum: Number.MAX_VALUE },
    'center',
    definitions,
    'Center',
  );
  expect(property).toEqual({ type: 'f64', valid: ['self.center.is_finite()'] });
});

test.each([
  { minimum: -1e300, maximum: Number.MAX_VALUE },
  { minimum: -Number.MAX_VALUE, maximum: 1e300 },
  { exclusiveMinimum: -Number.MAX_VALUE, maximum: Number.MAX_VALUE },
])('retains narrower or exclusive bounds: %j', (bounds) => {
  const property = rustProperty({ type: 'number', ...bounds }, 'center', definitions, 'Center');
  expect(property.valid.join(' ')).toContain('.contains(&self.center)');
  expect(property.valid).not.toContain('self.center.is_finite()');
});
