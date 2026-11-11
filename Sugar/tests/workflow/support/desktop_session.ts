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
/** Read the frozen portable fixture through production validators for frontend behavioral proofs. */
import { readFileSync } from 'node:fs';
import type {
  SessionCheckpoint,
  SessionFence,
} from '../../../integrations/contracts/SessionProtocol.generated.js';
import { decodeDesktopCheckpoint } from '../../../integrations/shared/SessionDecode.js';
import { isSessionFence } from '../../../integrations/shared/SessionValidation.generated.js';

/** Supply one admitted checkpoint and its matching frozen observation fence. */
export function desktopSessionFixture(): {
  checkpoint: SessionCheckpoint;
  fence: SessionFence;
  origin: string;
} {
  const vectors: unknown = JSON.parse(
    readFileSync('integrations/contracts/session_vectors.json', 'utf8'),
  );
  if (
    !vectors ||
    typeof vectors !== 'object' ||
    !('checkpoints' in vectors) ||
    !Array.isArray(vectors.checkpoints) ||
    !('currentFence' in vectors) ||
    !isSessionFence(vectors.currentFence) ||
    !('origin' in vectors) ||
    typeof vectors.origin !== 'string'
  )
    throw new Error('Shared desktop-session vectors are unavailable.');
  const first: unknown = vectors.checkpoints[0];
  if (!first || typeof first !== 'object' || !('value' in first))
    throw new Error('Shared checkpoint fixture is unavailable.');
  const checkpoint = decodeDesktopCheckpoint(JSON.stringify(first.value));
  if (!checkpoint) throw new Error('Shared checkpoint fixture is invalid.');
  return { checkpoint, fence: vectors.currentFence, origin: vectors.origin };
}
