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
/** Prove the Photoshop panel conforms and still answers as its newest recorded profile. */
import { expect, test } from '@jest/globals';
import { PHOTOSHOP_HOST_DESCRIPTION } from '../../../integrations/photoshop/src/application/PhotoshopHostDescription.js';
import { photoshopPresentation } from '../../../integrations/photoshop/src/infrastructure/PhotoshopTheme.js';
import { runHostConformance } from '../../../integrations/shared/conformance/HostConformance.js';
import { inMemoryHostChannel } from '../../../integrations/shared/conformance/InMemoryHostChannel.js';
import { recordHostProfile } from '../../../integrations/shared/conformance/HostProfiles.js';
import {
  canvasWithoutSurface,
  masksWithoutDocument,
  conformanceSession as session,
} from '../../editor_integrations/support/ConformanceHosts.js';
import {
  currentRelease,
  expectNewestProfile,
} from '../../editor_integrations/support/RecordedProfiles.js';

test('the Photoshop panel passes the conformance kit and matches its newest recorded profile', async () => {
  // The panel's canvas server has no output surface and no document is open, so no check
  // reaches the native editor.
  const report = await runHostConformance(
    inMemoryHostChannel({
      session,
      description: PHOTOSHOP_HOST_DESCRIPTION,
      presentation: photoshopPresentation('dark'),
      handlers: (send) => [canvasWithoutSurface(send), masksWithoutDocument(send)],
    }),
    { session },
  );
  expect(report.results.every(({ outcome }) => outcome !== undefined)).toBe(true);
  expectNewestProfile(recordHostProfile('photoshop', currentRelease(), session, report));
});
