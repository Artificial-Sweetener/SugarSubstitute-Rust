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
/** Prove an expanded cube stack sits exactly one card gap from the cards; a compact one keeps its spacing. */
import { afterEach, expect, test } from '@jest/globals';
import {
  CUBE_STACK_EXPANDED_WIDTH,
  cubeStackExpandedReservedWidth,
  cubeStackReservedWidth,
} from '../../../frontend/comfyui/ui/surface/stack/CubeStackMetrics.js';
import {
  MASONRY_STACK_GAP,
  ensureWorkflowMasonryStyles,
} from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryStyles.js';
import { WORKFLOW_MASONRY_GAP } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryLayout.js';

afterEach(() => document.head.replaceChildren());

test('an expanded stack reserves up to its own edge, floating or framed', () => {
  // A floating rail sits 12 px in; the reservation ends at its right edge.
  expect(cubeStackExpandedReservedWidth(false, false)).toBe(12 + CUBE_STACK_EXPANDED_WIDTH);
  expect(cubeStackExpandedReservedWidth(false, true)).toBe(CUBE_STACK_EXPANDED_WIDTH);
  expect(cubeStackExpandedReservedWidth(true, false)).toBe(CUBE_STACK_EXPANDED_WIDTH);
  // The compact stack's reservation is unchanged.
  expect(cubeStackReservedWidth(true, false, false)).toBe(82);
});

test('beside an expanded stack the masonry leaves its card gap, not its inline padding', () => {
  ensureWorkflowMasonryStyles(document);
  const css = [...document.head.querySelectorAll('style')]
    .map((style) => style.textContent)
    .join('')
    .replace(/\s+/g, ' ');
  expect(css).toContain(
    'calc(var(--sc-masonry-stack-gap,var(--sc-masonry-inline-padding,20px)) + var(--sc-masonry-stack-width,0px))',
  );
});

test('the stack gap is the card gap in pixels, since the gap token is a unitless number', () => {
  // A unitless token inside calc(... + ...px) would make the whole padding invalid, so the cards
  // would lose their inset and sit under the expanded stack.
  expect(String(WORKFLOW_MASONRY_GAP)).toMatch(/^\d+$/);
  expect(MASONRY_STACK_GAP).toBe('calc(var(--sugarcubes-masonry-gap,8) * 1px)');
});
