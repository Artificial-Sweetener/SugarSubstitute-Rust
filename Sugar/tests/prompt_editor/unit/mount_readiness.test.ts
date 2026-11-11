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
/** Require a mounted native layout publication before completing prompt construction. */
import { PromptMountReadiness } from '../../../frontend/comfyui/ui/prompt_editor/PromptMountReadiness.js';

test('waits for mounted native output without requesting measurements or owning live geometry', async () => {
  let completed = false;
  const ready = new PromptMountReadiness();
  const pending = ready.wait().then(() => {
    completed = true;
  });
  ready.publish(0);
  await Promise.resolve();
  expect(completed).toBe(false);
  ready.publish(320);
  await pending;
  expect(completed).toBe(true);
  await ready.wait();
  // Responsive sizing remains native after construction; it cannot reopen the component lifetime.
  ready.publish(280);
  await ready.wait();
  expect(completed).toBe(true);
});

test('retirement releases obsolete construction and rejects later native publications', async () => {
  const ready = new PromptMountReadiness();
  const pending = ready.wait();
  ready.dispose();
  await pending;
  ready.publish(320);
  await ready.wait();
});
