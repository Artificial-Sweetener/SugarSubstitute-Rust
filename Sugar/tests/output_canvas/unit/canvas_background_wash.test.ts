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
/** Prove the canvas takes the masonry's wash from the same tokens, so the two never drift apart. */
import { afterEach, expect, test } from '@jest/globals';
import { ensureCanvasViewportStyles } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasViewportStyles.js';
import { ensureCanvasWorkspaceStyles } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasWorkspaceStyles.js';
import { desktopFrameCss } from '../../../frontend/comfyui/ui/surface/frame/DesktopFrameStyles.js';

afterEach(() => document.head.replaceChildren());

const SHARED =
  'var(--sugarcubes-canvas-background,var(--sugarcubes-masonry-surface,var(--comfy-menu-bg,#202020)))';

test('the canvas viewport and pane use the masonry surface wherever no frame wash applies', () => {
  ensureCanvasViewportStyles(document);
  ensureCanvasWorkspaceStyles(document);
  const css = [...document.head.querySelectorAll('style')]
    .map((style) => style.textContent)
    .join('');
  expect(css).toContain(
    `.sc-output-viewport { position:absolute; inset:0; overflow:hidden; touch-action:none;`,
  );
  expect(css.split(`background:${SHARED}`).length - 1).toBeGreaterThanOrEqual(2);
  expect(css).not.toContain('#151515');
});

test('inside the desktop frame the canvas takes the masonry backdrop’s own body wash', () => {
  const css = desktopFrameCss.replace(/\s+/g, ' ');
  expect(css).toContain('--sugarcubes-canvas-background:var(--sc-desktop-body-wash);');
  expect(css).toContain(
    ':where(#graph-canvas,.sugarcubes-workflow-masonry__backdrop) { background:var(--sc-desktop-body-wash); }',
  );
});
