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
/** Prove the layer, geometry and editing capability forms stay closed and mutually exclusive. */
import { WEB_CANVAS } from '../../../integrations/contracts/CanvasHost.js';
import {
  isCanvasCapabilities,
  isCanvasEditingCapabilities,
  isCanvasGeometryCapabilities,
  isCanvasLayerCapabilities,
  isCanvasMessage,
} from '../../../integrations/shared/CanvasValidation.generated.js';
import {
  canvasWireMessage,
  invalidCanvasWireMessages,
  readCanvasWireVectors,
} from '../support/canvas_wire_vectors.js';

const vectors = readCanvasWireVectors('canvas_capability_vectors.json');
const forms = [
  isCanvasLayerCapabilities,
  isCanvasGeometryCapabilities,
  isCanvasEditingCapabilities,
];

/** Read the capabilities a valid context reply vector advertises. */
function capabilities(name: string): unknown {
  const message = canvasWireMessage(vectors, name);
  if (!isCanvasMessage(message) || message.type !== 'context.reply')
    throw new Error('Invalid context reply vector.');
  return message.context.canvas.capabilities;
}

test('admits each advertised form and refuses every hybrid or extended form', () => {
  for (const [name, message] of Object.entries(vectors.valid))
    expect([name, isCanvasMessage(message)]).toEqual([name, true]);
  for (const { name, message } of invalidCanvasWireMessages(vectors))
    expect([name, isCanvasMessage(message)]).toEqual([name, false]);
});

test.each([
  ['layer', 0],
  ['geometry', 1],
  ['editing', 2],
])('the %s form matches exactly one capability variant', (name, index) => {
  const value = capabilities(name);
  expect(isCanvasCapabilities(value)).toBe(true);
  expect(forms.map((form) => form(value))).toEqual(forms.map((_, form) => form === index));
});

test('the web canvas and existing editor descriptors keep the layer and geometry forms', () => {
  expect(isCanvasLayerCapabilities(WEB_CANVAS.capabilities)).toBe(true);
  expect(
    isCanvasGeometryCapabilities({
      displayOutput: false,
      insertImageLayer: true,
      positionImageLayer: true,
      geometry: true,
    }),
  ).toBe(true);
});
