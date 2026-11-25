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
/** The queue fly-out hangs below its button and never grows over it; a long list scrolls inside it. */
import { expect, test } from '@jest/globals';
import { generationQueueFlyoutMaxHeight } from '../../../frontend/comfyui/ui/surface/frame/queue/GenerationQueueFlyout.js';
import {
  GENERATION_QUEUE_ARROW_HEIGHT,
  GENERATION_QUEUE_FLYOUT_MAX_HEIGHT,
} from '../../../frontend/comfyui/ui/surface/frame/queue/GenerationQueueViewStyles.js';
import { floatingPosition } from '../../../frontend/comfyui/ui/menus/FloatingSurface.js';
import { GenerationQueueView } from '../../../frontend/comfyui/ui/surface/frame/queue/GenerationQueueView.js';

test('in a tall window the fly-out keeps its own greatest height', () => {
  expect(generationQueueFlyoutMaxHeight(30, 1100)).toBe(GENERATION_QUEUE_FLYOUT_MAX_HEIGHT);
});

test('in a short window it takes only the room below the button, so it never turns over the button', () => {
  const buttonBottom = 30;
  const windowHeight = 300;
  const height = generationQueueFlyoutMaxHeight(buttonBottom, windowHeight);
  expect(height).toBeLessThan(GENERATION_QUEUE_FLYOUT_MAX_HEIGHT);
  // Placed below the button at that height, the surface stays below it.
  const anchorBottom = buttonBottom + GENERATION_QUEUE_ARROW_HEIGHT;
  const placed = floatingPosition(
    { kind: 'below', bounds: { left: 600, right: 960, top: 0, bottom: anchorBottom } },
    360,
    height,
    1280,
    windowHeight,
  );
  expect(placed.top).toBeGreaterThanOrEqual(anchorBottom);
  expect(placed.top + height).toBeLessThanOrEqual(windowHeight);
});

test('a window with no room below the button gives the fly-out no height rather than a negative one', () => {
  expect(generationQueueFlyoutMaxHeight(30, 20)).toBe(0);
});

test('the list scrolls with the shared overlay scroll bar, as the prompt box does', () => {
  for (const surface of ['flyout', 'panel'] as const) {
    const view = new GenerationQueueView(document, surface, {
      act: () => undefined,
      open: () => undefined,
      menu: () => undefined,
      more: () => undefined,
    });
    const rows = view.element.querySelector('.sc-generation-queue-rows');
    // Nothing opts the list out of the one scroll bar owner.
    expect(rows?.hasAttribute('data-sc-scrollbar-managed')).toBe(false);
    expect(rows?.closest('[data-sc-scrollbar-managed]')).toBeNull();
  }
});
