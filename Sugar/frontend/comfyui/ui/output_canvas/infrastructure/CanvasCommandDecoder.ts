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
/** Validate versioned external intents against an explicitly authorized attachment. */

import type {
  OutputSourceId,
  OutputResultId,
  WorkflowSessionId,
} from '../../execution/domain/ExecutionIdentity.js';
import { sessionUuid } from '../../core/embedded/SessionRuntime.js';
import { isRecord } from '../../types/common.js';
import type {
  CanvasCapabilities,
  CanvasCommand,
  CanvasCommandRequest,
} from '../domain/CanvasViewPort.js';
import type { CanvasSelection } from '../domain/CanvasViewState.js';
import type { CanvasRevealTarget } from '../domain/CanvasReveal.js';
import type { CanvasMediaTarget } from '../domain/CanvasMedia.js';
import { authorizeCanvasMessage, type CanvasMessageAuthority } from './CanvasMessageAuthority.js';

export interface CanvasCommandAuthority extends CanvasMessageAuthority {
  readonly capabilities: CanvasCapabilities;
}

/** Reject wrong origins, versions, generations, capabilities, and malformed parameters. */
export function decodeCanvasCommandRequest(
  value: unknown,
  origin: unknown,
  authority: CanvasCommandAuthority,
): CanvasCommandRequest | null {
  if (!authorizeCanvasMessage(value, origin, authority)) return null;
  if (
    !nonnegativeInteger(value.requestId) ||
    value.requestId === 0 ||
    !nonnegativeInteger(value.expectedRevision)
  )
    return null;
  const command = decodeCommand(value.command);
  if (
    !command ||
    authority.capabilities.schema !== 1 ||
    !authority.capabilities.commands.includes(command.type)
  )
    return null;
  return Object.freeze({
    schema: 1,
    address: authority.address,
    requestId: value.requestId,
    expectedRevision: value.expectedRevision,
    command,
  });
}

/** Limit numeric fields to finite safe values before crossing the adapter boundary. */
function nonnegativeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

/** Bound source identities without interpreting their full nested execution paths. */
function sourceIdentity(value: unknown): value is OutputSourceId {
  return (
    typeof value === 'string' &&
    value.startsWith('source:') &&
    value.length > 7 &&
    value.length <= 4096
  );
}

/** Normalize selections into fresh immutable values without retaining external objects. */
function decodeSelection(value: unknown): CanvasSelection | null {
  if (!isRecord(value) || !sourceIdentity(value.sourceId) || !nonnegativeInteger(value.batchIndex))
    return null;
  return Object.freeze({ sourceId: value.sourceId, batchIndex: value.batchIndex });
}

/** Decode only supported bounded intents, without copying arbitrary payload fields. */
function decodeCommand(value: unknown): CanvasCommand | null {
  if (!isRecord(value)) return null;
  switch (value.type) {
    case 'reveal': {
      const target = decodeRevealTarget(value.target);
      return target ? Object.freeze({ type: 'reveal', target }) : null;
    }
    case 'fit':
    case 'native':
    case 'swap':
    case 'retry':
    case 'admit-large-image':
      return Object.freeze({ type: value.type });
    case 'select':
    case 'select-source': {
      const selection = decodeSelection(value.selection);
      return selection ? Object.freeze({ type: value.type, selection }) : null;
    }
    case 'select-comparison': {
      const selection = decodeSelection(value.selection);
      return selection ? Object.freeze({ type: 'select-comparison', selection }) : null;
    }
    case 'select-preview':
      return sourceIdentity(value.sourceId)
        ? Object.freeze({ type: 'select-preview', sourceId: value.sourceId })
        : null;
    case 'select-scene':
      return value.scene === null ||
        (typeof value.scene === 'string' && value.scene.length > 0 && value.scene.length <= 1000)
        ? Object.freeze({ type: 'select-scene', scene: value.scene })
        : null;
    case 'batch':
      return nonnegativeInteger(value.index)
        ? Object.freeze({ type: 'batch', index: value.index })
        : null;
    case 'follow':
      return value.mode === 'auto' || value.mode === 'manual'
        ? Object.freeze({ type: 'follow', mode: value.mode })
        : null;
    case 'overview':
    case 'link':
    case 'comparison':
      return typeof value.enabled === 'boolean'
        ? Object.freeze({ type: value.type, enabled: value.enabled })
        : null;
    case 'zoom':
      return typeof value.factor === 'number' &&
        Number.isFinite(value.factor) &&
        value.factor >= 0.01 &&
        value.factor <= 100
        ? Object.freeze({ type: 'zoom', factor: value.factor })
        : null;
    case 'pan':
      return value.units === 'viewport' &&
        typeof value.x === 'number' &&
        Number.isFinite(value.x) &&
        Math.abs(value.x) <= 10 &&
        typeof value.y === 'number' &&
        Number.isFinite(value.y) &&
        Math.abs(value.y) <= 10
        ? Object.freeze({ type: 'pan', x: value.x, y: value.y, units: 'viewport' })
        : null;
    case 'wipe':
      return (value.orientation === 'vertical' || value.orientation === 'horizontal') &&
        typeof value.split === 'number' &&
        Number.isFinite(value.split) &&
        value.split >= 0 &&
        value.split <= 1
        ? Object.freeze({ type: 'wipe', orientation: value.orientation, split: value.split })
        : null;
    case 'open-media':
      return value.target === 'primary' || value.target === 'secondary'
        ? Object.freeze({ type: value.type, target: value.target })
        : null;
    case 'copy-media': {
      const target = decodeMediaTarget(value.target);
      const format = value.format === undefined ? 'preferred' : value.format;
      return target && (format === 'preferred' || format === 'png' || format === 'jpeg')
        ? Object.freeze({ type: 'copy-media', target, format })
        : null;
    }
    case 'file-media': {
      const target = decodeMediaTarget(value.target);
      return target ? Object.freeze({ type: 'file-media', target }) : null;
    }
    case 'photoshop-media': {
      const target = decodeMediaTarget(value.target);
      return target ? Object.freeze({ type: 'photoshop-media', target }) : null;
    }
    case 'photoshop-all-media':
      return Object.freeze({ type: value.type });
    default:
      return null;
  }
}

/** Admit immutable output identities without accepting arbitrary paths or recycled batch positions. */
function decodeMediaTarget(value: unknown): CanvasMediaTarget | null {
  if (value === 'primary' || value === 'secondary') return value;
  return isRecord(value) && sourceIdentity(value.sourceId) && resultIdentity(value.itemId)
    ? Object.freeze({ sourceId: value.sourceId, itemId: value.itemId })
    : null;
}

/** Narrow external immutable-image identifiers before they reach selection or resource owners. */
function decodeRevealTarget(value: unknown): CanvasRevealTarget | null {
  if (
    !isRecord(value) ||
    !workflowIdentity(value.sessionId) ||
    !resultIdentity(value.itemId) ||
    !sourceIdentity(value.sourceId) ||
    !nonnegativeInteger(value.generation) ||
    !nonnegativeInteger(value.runOrder) ||
    !nonnegativeInteger(value.batchIndex)
  )
    return null;
  return Object.freeze({
    sessionId: value.sessionId,
    itemId: value.itemId,
    sourceId: value.sourceId,
    generation: value.generation,
    runOrder: value.runOrder,
    batchIndex: value.batchIndex,
  });
}

/** Narrow a workflow token without trusting a browser-provided object or arbitrary cast. */
function workflowIdentity(value: unknown): value is WorkflowSessionId {
  return (
    typeof value === 'string' &&
    value.startsWith('workflow:') &&
    value.length > 9 &&
    value.length <= 4096
  );
}

/**
 * Bound an immutable result identity independently of its internal encoding: a live job's `result:`
 * identity, or the durable artifact UUID an image restored from a desktop session keeps.
 */
function resultIdentity(value: unknown): value is OutputResultId {
  return (
    typeof value === 'string' &&
    ((value.startsWith('result:') && value.length > 7 && value.length <= 16384) ||
      sessionUuid(value))
  );
}
