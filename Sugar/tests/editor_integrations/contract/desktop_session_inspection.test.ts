/** @jest-environment node */
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
/** Preserve the actual canvas inspection owner's physical magnification through portable DTOs. */
import { readFileSync } from 'node:fs';
import {
  captureCanvasInspection,
  projectCanvasInspection,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasInspection.js';
import {
  centeredCanvasCamera,
  settleCanvasCamera,
  type CanvasCamera,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasCamera.js';
import { decodeDesktopCheckpoint } from '../../../integrations/shared/SessionDecode.js';

const fixture = JSON.parse(readFileSync('integrations/contracts/session_vectors.json', 'utf8')) as {
  readonly checkpoints: readonly { readonly value: unknown }[];
};
const previous = {
  image: { width: 640, height: 480 },
  viewport: { width: 800, height: 600 },
  density: 1.5,
};
const next = {
  image: { width: 640, height: 480 },
  viewport: { width: 1000, height: 700 },
  density: 2,
};

test.each(['fit', 'native', 'custom'] as const)(
  'roundtrips %s inspection through its authoritative physical-pixel projection',
  (mode) => {
    const camera: CanvasCamera =
      mode === 'fit'
        ? centeredCanvasCamera(previous)
        : settleCanvasCamera(
            { scale: mode === 'native' ? 1 / previous.density : 2, x: -140, y: -110, mode },
            previous,
          );
    const inspection = captureCanvasInspection(camera, previous);
    const value = decodeDesktopCheckpoint(JSON.stringify(fixture.checkpoints[0]?.value));
    const entry = value?.entries[0];
    const output = entry?.outputs[0];
    if (!value || !entry || !output) throw new Error('Missing accepted final artifact fixture');
    const checkpoint = {
      ...value,
      entries: [
        {
          ...entry,
          view: {
            ...entry.view,
            sharedInspection: { mode: inspection.mode, ...inspection.region },
            cameras: [
              { outputId: output.id, inspection: { mode: inspection.mode, ...inspection.region } },
            ],
          },
        },
        ...value.entries.slice(1),
      ],
    };
    const restored = decodeDesktopCheckpoint(JSON.stringify(checkpoint));
    const saved = restored?.entries[0]?.view.cameras[0]?.inspection;
    if (!saved) throw new Error('Portable checkpoint rejected the authoritative inspection');
    const projected = projectCanvasInspection({ mode: saved.mode, region: saved }, next);
    expect(projected).toEqual(projectCanvasInspection(inspection, next));
    if (mode === 'fit') expect(projected).toEqual(centeredCanvasCamera(next));
    else expect(projected.scale * next.density).toBeCloseTo(camera.scale * previous.density);
  },
);
