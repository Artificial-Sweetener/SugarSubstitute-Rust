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
/** Supply deterministic compositor completions without replacing workspace layout behavior. */
import { jest } from '@jest/globals';

/** Install only the browser Animation boundary and expose exact completion/cancellation control. */
export function installCanvasWorkspaceAnimation(element: HTMLElement) {
  const effects: {
    frames: Keyframe[];
    options: KeyframeAnimationOptions;
    cancel: ReturnType<typeof jest.fn>;
    finish(): void;
    fail(error: Error): void;
  }[] = [];
  const animate = jest.fn((frames: Keyframe[], options: KeyframeAnimationOptions) => {
    let finish = (): void => undefined;
    let fail = (_error: Error): void => undefined;
    const finished = new Promise<void>((resolve, reject) => {
      finish = resolve;
      fail = reject;
    });
    const effect = {
      frames,
      options,
      finished,
      cancel: jest.fn(() => fail(new Error('Canceled'))),
      finish: () => finish(),
      fail: (error: Error) => fail(error),
    };
    effects.push(effect);
    return effect;
  });
  Object.defineProperty(element, 'animate', { configurable: true, value: animate });
  return { effects, animate };
}
