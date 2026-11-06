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
/** Keep current and saved baseline graph data inside one UTF-8 document admission budget. */
import { readFileSync } from 'node:fs';
import type { SessionDocument } from '../../../integrations/contracts/SessionProtocol.generated.js';
import { decodeDesktopCheckpoint } from '../../../integrations/shared/SessionDecode.js';
import { sessionDocumentError } from '../../../integrations/shared/SessionDocuments.js';
import {
  SESSION_DOCUMENT_BYTES,
  utf8Bytes,
} from '../../../integrations/shared/SessionPrimitives.js';

/** Obtain an admitted occurrence without inventing a second document fixture contract. */
function document(): SessionDocument {
  const fixture = JSON.parse(
    readFileSync('integrations/contracts/session_vectors.json', 'utf8'),
  ) as { readonly checkpoints: readonly { readonly value: unknown }[] };
  const value = decodeDesktopCheckpoint(JSON.stringify(fixture.checkpoints[0]?.value));
  const result = value?.entries[0]?.document;
  if (!result) throw new Error('Missing admitted document fixture');
  return result;
}

test('combined multibyte UTF-8 admission precedes decoding either current or baseline graph', () => {
  const half = '🍬'.repeat(SESSION_DOCUMENT_BYTES / 8);
  expect(
    sessionDocumentError({ ...document(), graphJson: half, baselineGraphJson: `${half}🍬` }),
  ).toBe('too-large');
});

test('the complete current/baseline envelope admits exactly32MiB and rejects one more byte', () => {
  const value = { ...document(), graphJson: '{"padding":""}', baselineGraphJson: '{"nodes":[]}' };
  const overhead = utf8Bytes(JSON.stringify(value));
  const padding = 'x'.repeat(SESSION_DOCUMENT_BYTES - overhead);
  value.graphJson = JSON.stringify({ padding });
  expect(utf8Bytes(JSON.stringify(value))).toBe(SESSION_DOCUMENT_BYTES);
  expect(sessionDocumentError(value)).toBeNull();
  value.graphJson = JSON.stringify({ padding: `${padding}x` });
  expect(sessionDocumentError(value)).toBe('too-large');
});

test.each(['current', 'baseline'] as const)(
  '%s graph counts escaped Unicode against the same encoded envelope budget',
  (slot) => {
    const value = { ...document(), graphJson: '{"nodes":[]}', baselineGraphJson: '{"nodes":[]}' };
    const empty = '{"padding":""}';
    if (slot === 'current') value.graphJson = empty;
    else value.baselineGraphJson = empty;
    const available = SESSION_DOCUMENT_BYTES - utf8Bytes(JSON.stringify(value));
    const token = '\\ud83c\\udf6c';
    const encodedTokenBytes = utf8Bytes(JSON.stringify(token)) - 2;
    const padding =
      token.repeat(Math.floor(available / encodedTokenBytes)) +
      'x'.repeat(available % encodedTokenBytes);
    const graph = '{"padding":"' + padding + '"}';
    if (slot === 'current') value.graphJson = graph;
    else value.baselineGraphJson = graph;
    expect(utf8Bytes(JSON.stringify(value))).toBe(SESSION_DOCUMENT_BYTES);
    expect(sessionDocumentError(value)).toBeNull();
    if (slot === 'current') value.graphJson = graph.slice(0, -2) + 'x"}';
    else value.baselineGraphJson = graph.slice(0, -2) + 'x"}';
    expect(sessionDocumentError(value)).toBe('too-large');
  },
);
