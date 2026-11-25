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
/** The Substitute frame shows a run's progress and outcome itself, so Comfy's queue notices are hidden in it. */
import { afterEach, expect, test } from '@jest/globals';
import { substituteFrameCss } from '../../../frontend/comfyui/ui/surface/frame/SubstituteFrameStyles.js';

afterEach(() => {
  document.documentElement.removeAttribute('data-sc-substitute-frame');
  document.head.replaceChildren();
  document.body.replaceChildren();
});

/** Mount Comfy's queue notice host as its frontend renders it. */
function mountNotice(): HTMLElement {
  const style = document.createElement('style');
  style.textContent = substituteFrameCss;
  document.head.append(style);
  const host = document.createElement('div');
  host.className = 'flex justify-end';
  host.setAttribute('role', 'status');
  host.dataset.testid = 'queue-notification-banner';
  host.textContent = 'Job queued';
  document.body.append(host);
  return host;
}

test('no queue notice (queuing, queued, failed, completed) shows in the Substitute frame', () => {
  document.documentElement.setAttribute('data-sc-substitute-frame', '');
  expect(getComputedStyle(mountNotice()).display).toBe('none');
});

test('the browser keeps Comfy queue notices', () => {
  expect(getComputedStyle(mountNotice()).display).not.toBe('none');
});
