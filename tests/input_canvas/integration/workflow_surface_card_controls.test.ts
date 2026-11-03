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
/** Prove the "Visual opacity" field on mask cards through the native node-field owner. */
import { jest } from '@jest/globals';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';
import { nativePresentationWidgets } from '../../../frontend/comfyui/ui/surface/NativePresentationWidgets.js';
import {
  MaskOpacityField,
  maskOpacityLabel,
} from '../../../frontend/comfyui/ui/input_canvas/presentation/MaskOpacityField.js';
import type { InputSurfaceCardPort } from '../../../frontend/comfyui/ui/input_canvas/application/InputEditorPorts.js';
import { nativeInputFields } from '../support/native_input_fields.js';
import { MaskImportCommand } from '../../../frontend/comfyui/ui/input_canvas/presentation/MaskImportCommand.js';

afterEach(() => jest.useRealTimers());

/** Expose a card port whose opacity tests change like provisioning does. */
function card(initial: number | null) {
  let opacity = initial;
  const listeners = new Set<() => void>();
  const port: InputSurfaceCardPort = {
    activate: jest.fn(),
    hover: jest.fn(),
    hovered: () => null,
    subscribeHover: () => () => {},
    region: () => null,
    opacity: () => opacity,
    changeOpacity: jest.fn(),
    canImport: () => opacity !== null,
    importMasks: jest.fn(async () => {}),
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
  return {
    port,
    publish(next: number | null) {
      opacity = next;
      for (const listener of listeners) listener();
    },
  };
}

/** Find the leased opacity widget. */
function widget(node: ComfyNode) {
  return nativePresentationWidgets(node).find((item) => item.name.endsWith('mask-visual-opacity'));
}

test('the field appears only for provisioned masks and one slider drag becomes one change', () => {
  jest.useFakeTimers();
  const node: ComfyNode = { id: 1, type: 'LoadImageMask', widgets: [], inputs: [], outputs: [] };
  const surface = card(null);
  const field = new MaskOpacityField(
    surface.port,
    nativeInputFields(node),
    maskOpacityLabel(null),
    jest.fn(),
  );
  expect(widget(node)).toBeUndefined();
  surface.publish(0.5);
  const slider = widget(node);
  expect(slider).toMatchObject({ value: 50, label: 'Visual opacity' });
  for (const value of [40, 30, 25]) slider?.callback?.(value);
  expect(surface.port.changeOpacity).not.toHaveBeenCalled();
  jest.advanceTimersByTime(250);
  expect(surface.port.changeOpacity).toHaveBeenCalledTimes(1);
  expect(surface.port.changeOpacity).toHaveBeenCalledWith(0.25);
  slider?.callback?.(80);
  field.dispose();
  expect(surface.port.changeOpacity).toHaveBeenLastCalledWith(0.8);
  expect(widget(node)).toBeUndefined();
  expect(maskOpacityLabel('mask')).toBe('mask Visual opacity');
});

test('"Choose mask" appears for provisioned regional loaders and imports every chosen picture', async () => {
  const surface = card(null);
  const report = jest.fn();
  const command = new MaskImportCommand(document, surface.port, report);
  document.body.append(command.element);
  expect(command.element.hidden).toBe(true);
  surface.publish(0.5);
  expect(command.element.hidden).toBe(false);
  expect(command.element.textContent).toContain('Choose mask');
  const input = command.element.querySelector<HTMLInputElement>('input[type="file"]');
  expect(input?.multiple).toBe(true);
  const files = [new File(['a'], 'a.png'), new File(['b'], 'b.png')];
  Object.defineProperty(input, 'files', { configurable: true, value: files });
  input?.dispatchEvent(new Event('change'));
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(surface.port.importMasks).toHaveBeenCalledWith(files);
  jest.mocked(surface.port.importMasks).mockRejectedValueOnce(new Error('Undecodable.'));
  input?.dispatchEvent(new Event('change'));
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(report).toHaveBeenCalledWith(new Error('Undecodable.'));
  command.dispose();
  expect(command.element.isConnected).toBe(false);
});
