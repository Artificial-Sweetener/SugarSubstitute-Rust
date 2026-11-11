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
/** The real host-update/state owners consume a literal supported measurement and input-reservation seam. */
import type { EditorView } from '@codemirror/view';
import { hostFixture } from './host-document-fixture.ts';

/** Native geometry requests are derived from the public CodeMirror method. */
type Measure = NonNullable<Parameters<EditorView['requestMeasure']>[0]>;

/** No native input implementation or browser view is substituted; only command reservation and measurement delivery are controlled. */
export function checkpointFixture(source = 'initial') {
  const fixture = hostFixture(source);
  const measurements: Measure[] = [];
  let finished = 0,
    cancelled = 0;
  Object.assign(fixture.transport, {
    plugin: () => ({
      hasCompositionInput: () => false,
      reserveCommand: () => ({
        finish: () => {
          finished++;
        },
        cancel: () => {
          cancelled++;
        },
      }),
    }),
    requestMeasure: (request: Measure) => {
      measurements.push(request);
    },
  });
  return {
    ...fixture,
    measurements,
    /** Deliver a supported measurement without browser geometry claims. */
    flush() {
      const view = fixture.transport as unknown as EditorView;
      for (const request of measurements.splice(0)) request.write?.(request.read(view), view);
    },
    /** Capture only through the actual integrated host owner. */
    async capture() {
      const pending = fixture.updates.captureDocument();
      this.flush();
      const result = await pending;
      if (result.status !== 'captured') throw new Error('CHECKPOINT_CAPTURE_REJECTED');
      return result.checkpoint;
    },
    counts: () => ({ finished, cancelled }),
    /** Controller teardown revokes handles before releasing the native view transport. */
    dispose() {
      fixture.updates.dispose();
      fixture.destroy();
    },
  };
}
