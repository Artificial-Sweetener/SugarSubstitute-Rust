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
/** Prove docked panels slide in and out with the shared motion, and never move with reduced motion. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { PanelReveal } from '../../../frontend/comfyui/ui/surface/motion/PanelReveal.js';
import { surfaceMotion } from '../../../frontend/comfyui/ui/surface/appearance/SurfaceMotionTokens.js';

afterEach(() => {
  document.body.replaceChildren();
  Reflect.deleteProperty(window, 'matchMedia');
});

/** A controllable stand-in for one Web Animation. */
class FakeAnimation extends EventTarget {
  playbackRate = 1;
  readonly cancel = jest.fn();
  reverse(): void {
    this.playbackRate = -1;
  }
  finishNow(): void {
    this.dispatchEvent(new Event('finish'));
  }
}

function panel(reduced = false) {
  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    value: (query: string) => ({ matches: reduced && query.includes('reduce') }),
  });
  const element = document.createElement('div');
  const animations: {
    frames: Keyframe[];
    options: KeyframeAnimationOptions;
    run: FakeAnimation;
  }[] = [];
  element.animate = ((frames: Keyframe[], options: KeyframeAnimationOptions) => {
    const run = new FakeAnimation();
    animations.push({ frames, options, run });
    return run as unknown as Animation;
  }) as typeof element.animate;
  document.body.append(element);
  return { element, animations };
}

test('a panel slides in from its edge with the shared enter motion', () => {
  const { element, animations } = panel();
  new PanelReveal().enter(element, 'bottom');
  expect(animations).toHaveLength(1);
  expect(animations[0]?.options.duration).toBe(surfaceMotion.duration);
  expect(animations[0]?.frames[0]).toMatchObject({ transform: 'translateY(100%)', opacity: 0 });
});

test('a panel slides out and is released only once the slide ends', () => {
  const { element, animations } = panel();
  const reveal = new PanelReveal();
  const done = jest.fn();
  reveal.exit(element, 'right', done);
  expect(animations[0]?.options.duration).toBe(surfaceMotion.exitDuration);
  expect(reveal.leaving(element)).toBe(true);
  expect(done).not.toHaveBeenCalled();
  animations[0]?.run.finishNow();
  expect(done).toHaveBeenCalledTimes(1);
  expect(reveal.leaving(element)).toBe(false);
});

test('opening a panel again while it slides out takes the slide back and keeps the panel', () => {
  const { element, animations } = panel();
  const reveal = new PanelReveal();
  const done = jest.fn();
  reveal.exit(element, 'bottom', done);
  reveal.enter(element, 'bottom');
  expect(animations).toHaveLength(1);
  expect(animations[0]?.run.playbackRate).toBe(-1);
  animations[0]?.run.finishNow();
  expect(done).not.toHaveBeenCalled();
});

test('reduced motion opens and closes panels at once', () => {
  const { element, animations } = panel(true);
  const reveal = new PanelReveal();
  const done = jest.fn();
  reveal.enter(element, 'right');
  reveal.exit(element, 'right', done);
  expect(animations).toHaveLength(0);
  expect(done).toHaveBeenCalledTimes(1);
});

test('disposal completes every exit still running', () => {
  const { element } = panel();
  const reveal = new PanelReveal();
  const done = jest.fn();
  reveal.exit(element, 'bottom', done);
  reveal.dispose();
  expect(done).toHaveBeenCalledTimes(1);
});
