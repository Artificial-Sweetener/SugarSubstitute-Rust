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
/** Safeguard pane slide endpoints, retained pixels, reversals and finite teardown. */
import { jest } from '@jest/globals';
import { BrowserCanvasWorkspaceMotion } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasWorkspaceMotion.js';
import { installCanvasWorkspaceAnimation } from '../support/CanvasWorkspaceMotionFixture.js';

/** Exercise the real motion owner against a controllable browser effect. */
function fixture() {
  const element = document.createElement('aside');
  document.body.append(element);
  const animation = installCanvasWorkspaceAnimation(element);
  const settled = jest.fn();
  const motion = new BrowserCanvasWorkspaceMotion(element, settled);
  return { element, ...animation, motion, settled };
}

afterEach(() => {
  jest.restoreAllMocks();
  document.body.replaceChildren();
});

test('slides in and retains outgoing pixels until closing finishes', async () => {
  const f = fixture();
  expect(f.element.hidden).toBe(true);
  const entering = f.motion.setVisible(true);
  expect(f.effects[0]?.frames).toEqual([
    { transform: 'translateX(100%)' },
    { transform: 'translateX(0)' },
  ]);
  expect(f.motion.present).toBe(true);
  f.effects[0]!.finish();
  await entering;
  expect(f.element.dataset.canvasMotion).toBeUndefined();
  const exiting = f.motion.setVisible(false);
  expect(f.element.hidden).toBe(false);
  expect(f.element.inert).toBe(true);
  expect(f.motion.present).toBe(true);
  f.effects[1]!.finish();
  await exiting;
  expect(f.element.hidden).toBe(true);
  expect(f.motion.present).toBe(false);
  expect(f.settled).toHaveBeenCalledTimes(2);
  f.motion.dispose();
});

test('reverses at the painted transform and ignores superseded completions', async () => {
  const f = fixture();
  f.motion.restore(true);
  const exiting = f.motion.setVisible(false);
  f.element.style.transform = 'matrix(1, 0, 0, 1, 180, 0)';
  const entering = f.motion.setVisible(true);
  expect(f.effects[1]?.frames[0]?.transform).toBe('matrix(1, 0, 0, 1, 180, 0)');
  await exiting;
  expect(f.element.hidden).toBe(false);
  expect(f.settled).not.toHaveBeenCalled();
  f.effects[1]!.finish();
  await entering;
  expect(f.settled).toHaveBeenCalledTimes(1);
  f.motion.dispose();
});

test('restores workflow visibility and cancels teardown without replaying effects', async () => {
  const f = fixture();
  f.motion.restore(true);
  expect(f.animate).not.toHaveBeenCalled();
  const exiting = f.motion.setVisible(false);
  f.motion.restore(true);
  await exiting;
  expect(f.element.hidden).toBe(false);
  expect(f.settled).not.toHaveBeenCalled();
  const entering = f.motion.setVisible(false);
  f.motion.dispose();
  await entering;
  expect(f.element.hidden).toBe(true);
  expect(f.motion.present).toBe(false);
  expect(f.settled).not.toHaveBeenCalled();
});

test('settles and reports unexpected compositor failure', async () => {
  const f = fixture();
  f.motion.restore(true);
  const exiting = f.motion.setVisible(false);
  f.effects[0]!.fail(new Error('GPU effect failed'));
  await expect(exiting).rejects.toThrow('Output canvas slide failed.');
  expect(f.element.hidden).toBe(true);
  expect(f.motion.present).toBe(false);
  expect(f.settled).toHaveBeenCalledTimes(1);
  f.motion.dispose();
});

test('settles immediately when compositor animation is unavailable', async () => {
  const element = document.createElement('aside');
  const motion = new BrowserCanvasWorkspaceMotion(element, jest.fn());
  await motion.setVisible(true);
  expect(element.hidden).toBe(false);
  expect(element.dataset.canvasMotion).toBeUndefined();
  await motion.setVisible(false);
  expect(element.hidden).toBe(true);
  motion.dispose();
});

test('respects reduced motion without retaining an animation or delaying closure', async () => {
  const f = fixture();
  const original = Object.getOwnPropertyDescriptor(window, 'matchMedia');
  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    value: () => ({ matches: true }),
  });
  try {
    await f.motion.setVisible(true);
    expect(f.element.hidden).toBe(false);
    await f.motion.setVisible(false);
    expect(f.element.hidden).toBe(true);
    expect(f.animate).not.toHaveBeenCalled();
    expect(f.motion.present).toBe(false);
  } finally {
    f.motion.dispose();
    if (original) Object.defineProperty(window, 'matchMedia', original);
    else Reflect.deleteProperty(window, 'matchMedia');
  }
});
