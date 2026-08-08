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
/** Verify safe, compact, theme-aware DOM model-pill presentation. */

import { resolveCubeModelTitle } from '../../frontend/comfyui/ui/cube/CubeModelTitlePresentation.js';
import {
  createCubeModelTitleElement,
  presentCubeModelTitle,
} from '../../frontend/comfyui/ui/surface/CubeModelPillDomRenderer.js';

test('renders literal model and name text with a complete accessible title', () => {
  const presentation = resolveCubeModelTitle({
    targetModel: '<Anima>',
    title: '<Anima>/<script>alert(1)</script>',
  });
  const element = createCubeModelTitleElement(document, presentation, {
    punchoutColor: 'var(--test-header)',
  });

  expect(element.getAttribute('aria-label')).toBe('<Anima>/<script>alert(1)</script>');
  expect(element.querySelector('[data-sugarcubes-model-pill]')?.textContent).toBe('<Anima>');
  expect(element.querySelector('.sugarcubes-model-title__name')?.textContent).toBe(
    '<script>alert(1)</script>',
  );
  expect(element.querySelector('script')).toBeNull();
  expect(element.style.getPropertyValue('--sugarcubes-model-pill-punchout')).toBe(
    'var(--test-header)',
  );
});

test('keeps compact pill geometry in the shared theme-token stylesheet', () => {
  createCubeModelTitleElement(
    document,
    resolveCubeModelTitle({ targetModel: 'Anima', title: 'Anima/Detailer' }),
  );
  const styles = document.getElementById('sugarcubes-model-pill-styles')?.textContent ?? '';

  expect(styles).toContain('font-size: 0.78em');
  expect(styles).toContain('padding: 0.08em 0.38em');
  expect(styles).toContain('--p-content-background');
  expect(styles).toContain('background: currentColor');
});

test('does not rewrite an unchanged owned title', () => {
  const host = document.createElement('span');
  const presentation = resolveCubeModelTitle({
    targetModel: 'Anima',
    title: 'Anima/Detailer',
  });

  expect(presentCubeModelTitle(host, presentation)).toBe(true);
  const child = host.firstElementChild;
  expect(presentCubeModelTitle(host, presentation)).toBe(false);
  expect(host.firstElementChild).toBe(child);
});
