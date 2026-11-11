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
/** Describe the existing view and platform boundaries supplied to one canvas attachment. */

import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { CanvasImageAllowances } from '../application/CanvasImageAllowances.js';
import type { CanvasImageLoadPort } from '../domain/CanvasImageResource.js';
import type { BrowserCanvasImagePayload } from './BrowserCanvasImageLoader.js';
import type { CanvasAttachmentOptions } from './CanvasAttachmentOptions.js';
import type { BrowserCanvasMediaSessions } from './BrowserCanvasMediaSessions.js';
import type { CanvasModelRuntime } from './ComfyCanvasModelRuntime.js';
import type { MemoryBudgetRegistry } from '../../core/memory/MemoryBudgetRegistry.js';

export interface BrowserCanvasAttachmentOptions extends CanvasAttachmentOptions {
  readonly allowances: CanvasImageAllowances;
  readonly loader: CanvasImageLoadPort<BrowserCanvasImagePayload, HTMLImageElement>;
  readonly logger: Pick<Console, 'error'>;
  /** Page allowance that counts this attachment's displayed images as pinned. */
  readonly memory: MemoryBudgetRegistry;
  readonly models?: CanvasModelRuntime;
  readonly mediaSessions?: BrowserCanvasMediaSessions;
  /** Build the host's validated full-fidelity file URL. */
  url(item: WorkflowOutputItem): string;
  /** Open supported media in a separate browser window. */
  open(item: WorkflowOutputItem): void;
  /** Dispose platform loading after the resource owner has canceled every lease. */
  disposeLoader(): void;
}
