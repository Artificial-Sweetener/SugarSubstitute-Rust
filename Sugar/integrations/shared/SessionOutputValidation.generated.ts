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
/** Generated protocol validation; edit the authoritative session.schema.json. */
import type * as Contract from '../contracts/SessionProtocol.generated.js';

import { isRecord } from './ContractRecord.js';
/** Validate a bounded SessionScope payload before dispatch. */
export function isSessionScope(value: unknown): value is Contract.SessionScope {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'installation') &&
    typeof value['installation'] === 'string' &&
    value['installation'].length <= 32 &&
    Object.hasOwn(value, 'profile') &&
    typeof value['profile'] === 'string' &&
    value['profile'].length <= 64 &&
    Object.hasOwn(value, 'workspace') &&
    typeof value['workspace'] === 'string' &&
    value['workspace'].length <= 36 &&
    Object.hasOwn(value, 'backend') &&
    typeof value['backend'] === 'string' &&
    value['backend'].length <= 36
  );
}
/** Validate a bounded SessionSource payload before dispatch. */
export function isSessionSource(value: unknown): value is Contract.SessionSource {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'id') &&
    typeof value['id'] === 'string' &&
    value['id'].length <= 36 &&
    Object.hasOwn(value, 'nodeId') &&
    typeof value['nodeId'] === 'string' &&
    value['nodeId'].length <= 256 &&
    Object.hasOwn(value, 'executionPath') &&
    typeof value['executionPath'] === 'string' &&
    value['executionPath'].length <= 4096 &&
    Object.hasOwn(value, 'label') &&
    typeof value['label'] === 'string' &&
    value['label'].length <= 4096 &&
    Object.hasOwn(value, 'graphRevision') &&
    typeof value['graphRevision'] === 'number' &&
    Number.isSafeInteger(value['graphRevision']) &&
    value['graphRevision'] >= 0 &&
    value['graphRevision'] <= 9007199254740991
  );
}
/** Validate a bounded SessionScene payload before dispatch. */
export function isSessionScene(value: unknown): value is Contract.SessionScene {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'groupId') &&
    typeof value['groupId'] === 'string' &&
    value['groupId'].length <= 36 &&
    Object.hasOwn(value, 'sceneId') &&
    typeof value['sceneId'] === 'string' &&
    value['sceneId'].length <= 36 &&
    Object.hasOwn(value, 'order') &&
    typeof value['order'] === 'number' &&
    Number.isSafeInteger(value['order']) &&
    value['order'] >= 0 &&
    value['order'] <= 8191 &&
    Object.hasOwn(value, 'count') &&
    typeof value['count'] === 'number' &&
    Number.isSafeInteger(value['count']) &&
    value['count'] >= 1 &&
    value['count'] <= 8192 &&
    Object.hasOwn(value, 'title') &&
    typeof value['title'] === 'string' &&
    value['title'].length <= 4096
  );
}
/** Validate a bounded SessionFile payload before dispatch. */
export function isSessionFile(value: unknown): value is Contract.SessionFile {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'storage') &&
    (value['storage'] === 'output' ||
      value['storage'] === 'input' ||
      value['storage'] === 'temp') &&
    Object.hasOwn(value, 'filename') &&
    typeof value['filename'] === 'string' &&
    value['filename'].length <= 255 &&
    Object.hasOwn(value, 'subfolder') &&
    typeof value['subfolder'] === 'string' &&
    value['subfolder'].length <= 4096 &&
    Object.hasOwn(value, 'managedArtifact') &&
    ((typeof value['managedArtifact'] === 'string' && value['managedArtifact'].length <= 256) ||
      value['managedArtifact'] === null)
  );
}
/** Validate a bounded SessionAsset payload before dispatch. */
export function isSessionAsset(value: unknown): value is Contract.SessionAsset {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'digest') &&
    typeof value['digest'] === 'string' &&
    value['digest'].length <= 64 &&
    Object.hasOwn(value, 'bytes') &&
    typeof value['bytes'] === 'number' &&
    Number.isSafeInteger(value['bytes']) &&
    value['bytes'] >= 1 &&
    value['bytes'] <= 8589934592 &&
    Object.hasOwn(value, 'extension') &&
    (value['extension'] === 'png' ||
      value['extension'] === 'jpg' ||
      value['extension'] === 'webp' ||
      value['extension'] === 'gif' ||
      value['extension'] === 'mp4' ||
      value['extension'] === 'webm') &&
    Object.hasOwn(value, 'media') &&
    (value['media'] === 'image' || value['media'] === 'video')
  );
}
/** Validate a bounded SessionOutput payload before dispatch. */
export function isSessionOutput(value: unknown): value is Contract.SessionOutput {
  return (
    isRecord(value) &&
    Object.keys(value).length === 20 &&
    Object.hasOwn(value, 'id') &&
    typeof value['id'] === 'string' &&
    value['id'].length <= 36 &&
    Object.hasOwn(value, 'entryId') &&
    typeof value['entryId'] === 'string' &&
    value['entryId'].length <= 36 &&
    Object.hasOwn(value, 'backend') &&
    typeof value['backend'] === 'string' &&
    value['backend'].length <= 36 &&
    Object.hasOwn(value, 'promptId') &&
    ((typeof value['promptId'] === 'string' && value['promptId'].length <= 256) ||
      value['promptId'] === null) &&
    Object.hasOwn(value, 'runId') &&
    typeof value['runId'] === 'string' &&
    value['runId'].length <= 36 &&
    Object.hasOwn(value, 'runOrder') &&
    typeof value['runOrder'] === 'number' &&
    Number.isSafeInteger(value['runOrder']) &&
    value['runOrder'] >= 0 &&
    value['runOrder'] <= 9007199254740991 &&
    Object.hasOwn(value, 'sourceId') &&
    typeof value['sourceId'] === 'string' &&
    value['sourceId'].length <= 36 &&
    Object.hasOwn(value, 'scene') &&
    (isSessionScene(value['scene']) || value['scene'] === null) &&
    Object.hasOwn(value, 'listIndex') &&
    typeof value['listIndex'] === 'number' &&
    Number.isSafeInteger(value['listIndex']) &&
    value['listIndex'] >= 0 &&
    value['listIndex'] <= 8191 &&
    Object.hasOwn(value, 'batchIndex') &&
    typeof value['batchIndex'] === 'number' &&
    Number.isSafeInteger(value['batchIndex']) &&
    value['batchIndex'] >= 0 &&
    value['batchIndex'] <= 8191 &&
    Object.hasOwn(value, 'imageIndex') &&
    typeof value['imageIndex'] === 'number' &&
    Number.isSafeInteger(value['imageIndex']) &&
    value['imageIndex'] >= 0 &&
    value['imageIndex'] <= 8191 &&
    Object.hasOwn(value, 'file') &&
    isSessionFile(value['file']) &&
    Object.hasOwn(value, 'assetDigest') &&
    ((typeof value['assetDigest'] === 'string' && value['assetDigest'].length <= 64) ||
      value['assetDigest'] === null) &&
    Object.hasOwn(value, 'digest') &&
    typeof value['digest'] === 'string' &&
    value['digest'].length <= 64 &&
    Object.hasOwn(value, 'bytes') &&
    typeof value['bytes'] === 'number' &&
    Number.isSafeInteger(value['bytes']) &&
    value['bytes'] >= 1 &&
    value['bytes'] <= 8589934592 &&
    Object.hasOwn(value, 'width') &&
    typeof value['width'] === 'number' &&
    Number.isSafeInteger(value['width']) &&
    value['width'] >= 1 &&
    value['width'] <= 131072 &&
    Object.hasOwn(value, 'height') &&
    typeof value['height'] === 'number' &&
    Number.isSafeInteger(value['height']) &&
    value['height'] >= 1 &&
    value['height'] <= 131072 &&
    Object.hasOwn(value, 'media') &&
    (value['media'] === 'image' || value['media'] === 'video') &&
    Object.hasOwn(value, 'mime') &&
    typeof value['mime'] === 'string' &&
    value['mime'].length <= 128 &&
    Object.hasOwn(value, 'metadataEmbedded') &&
    typeof value['metadataEmbedded'] === 'boolean'
  );
}
/** Validate a bounded SessionSubmission payload before dispatch. */
export function isSessionSubmission(value: unknown): value is Contract.SessionSubmission {
  return (
    isRecord(value) &&
    Object.keys(value).length === 8 &&
    Object.hasOwn(value, 'entryId') &&
    typeof value['entryId'] === 'string' &&
    value['entryId'].length <= 36 &&
    Object.hasOwn(value, 'backend') &&
    typeof value['backend'] === 'string' &&
    value['backend'].length <= 36 &&
    Object.hasOwn(value, 'promptId') &&
    typeof value['promptId'] === 'string' &&
    value['promptId'].length <= 256 &&
    Object.hasOwn(value, 'runId') &&
    typeof value['runId'] === 'string' &&
    value['runId'].length <= 36 &&
    Object.hasOwn(value, 'runOrder') &&
    typeof value['runOrder'] === 'number' &&
    Number.isSafeInteger(value['runOrder']) &&
    value['runOrder'] >= 0 &&
    value['runOrder'] <= 9007199254740991 &&
    Object.hasOwn(value, 'graphRevision') &&
    typeof value['graphRevision'] === 'number' &&
    Number.isSafeInteger(value['graphRevision']) &&
    value['graphRevision'] >= 0 &&
    value['graphRevision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'scene') &&
    (isSessionScene(value['scene']) || value['scene'] === null) &&
    Object.hasOwn(value, 'sources') &&
    Array.isArray(value['sources']) &&
    value['sources'].length >= 1 &&
    value['sources'].length <= 8192 &&
    value['sources'].every((item: unknown) => isSessionSource(item))
  );
}
