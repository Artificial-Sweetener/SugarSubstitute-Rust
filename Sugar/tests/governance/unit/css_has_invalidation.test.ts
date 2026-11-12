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
/**
 * Keep `class` attribute selectors out of `:has()` arguments.
 *
 * In ComfyUI, Chromium answers every class change under such a rule by re-testing the argument for each
 * ancestor, which cost each prompt-editor keystroke ~12 ms of style recalculation per DOM update.
 */
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { photoProWorkspaceCss } from '../../../frontend/comfyui/ui/surface/appearance/PhotoProWorkspaceStyles.js';
import {
  pluginFrameCss,
  pluginToolbarPropertiesToggle,
} from '../../../frontend/comfyui/ui/surface/frame/PluginFrameStyles.js';

const CLASS_ATTRIBUTE = /\[class\s*[~|^$*]?=/u;

/** Report whether any balanced `:has(...)` argument in the text selects on the class attribute. */
function selectsClassAttributeInHas(text: string): boolean {
  for (let start = text.indexOf(':has('); start >= 0; start = text.indexOf(':has(', start + 1)) {
    let depth = 0,
      end = start + 4;
    for (; end < text.length; end++) {
      if (text[end] === '(') depth++;
      else if (text[end] === ')' && --depth === 0) break;
    }
    if (CLASS_ATTRIBUTE.test(text.slice(start, end))) return true;
  }
  return false;
}

function sources(root: string): string[] {
  return readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(root, entry.name);
    return entry.isDirectory() ? sources(file) : entry.name.endsWith('.ts') ? [file] : [];
  });
}

test('recognizes class attribute selectors nested in :has() arguments', () => {
  expect(selectsClassAttributeInHas(':has(button > i:is([class~="icon-[x]"])) {}')).toBe(true);
  expect(selectsClassAttributeInHas('.a:has(> [class*="b"]) {}')).toBe(true);
  expect(selectsClassAttributeInHas(':has(button > i:is(.icon-\\[x\\])) {}')).toBe(false);
  expect(selectsClassAttributeInHas('[data-nav-id][class*="surface-selected"] {}')).toBe(false);
});

test('matches the properties toggle by class selectors in every toolbar stylesheet', () => {
  expect(pluginToolbarPropertiesToggle).toContain('.icon-\\[lucide--panel-right\\]');
  expect(pluginFrameCss).toContain(pluginToolbarPropertiesToggle);
  expect(photoProWorkspaceCss()).toContain(pluginToolbarPropertiesToggle);
});

test('authors no :has() argument that selects on the class attribute', () => {
  const offenders = sources('frontend').flatMap((file) =>
    readFileSync(file, 'utf8')
      .split(/\r?\n/u)
      .flatMap((line, index) => (selectsClassAttributeInHas(line) ? [`${file}:${index + 1}`] : [])),
  );
  expect(offenders).toEqual([]);
});
