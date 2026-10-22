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
/** Prove value refresh avoids layout reads while membership changes retain shared disclosure geometry. */
import { jest } from '@jest/globals';
import { NativeCardContentDisclosure } from '../../../frontend/comfyui/ui/surface/NativeCardContentDisclosure.js';
import { CardDisclosureMotion } from '../../../frontend/comfyui/ui/surface/motion/CardDisclosureMotion.js';

test('read geometry only when the upcoming native widget membership changes', () => {
  const target = document.createElement('div');
  const motion = new CardDisclosureMotion(target);
  const read = jest.spyOn(motion, 'captureHeight').mockReturnValue(128);
  const resize = jest.spyOn(motion, 'resize').mockImplementation((render) => render());
  const disclosure = new NativeCardContentDisclosure(target, motion);
  const render = jest.fn();
  disclosure.reconcile(['text'], render);
  disclosure.prepare(['text']);
  disclosure.reconcile(['text'], render);
  expect(read).not.toHaveBeenCalled();
  expect(resize).not.toHaveBeenCalled();
  disclosure.prepare(['text', 'strength']);
  expect(read).toHaveBeenCalledTimes(1);
  disclosure.reconcile(['text', 'strength'], render);
  expect(resize).toHaveBeenCalledWith(
    expect.any(Function),
    expect.objectContaining({ fromHeight: 128 }),
  );
  expect(render).toHaveBeenCalledTimes(3);
});
