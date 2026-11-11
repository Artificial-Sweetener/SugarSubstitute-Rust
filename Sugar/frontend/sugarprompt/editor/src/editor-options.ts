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
/** Public mount configuration delegates source/history and finite callbacks to one native lifetime. */
import type { SearchRequest, SearchHostIntent } from '../../services/src/search/contracts.js';
import type { TagLexicon } from '../../core/src/index.js';
import type { EditorSnapshot, EditorLayoutMetrics } from './contracts.js';
import type { HostConfiguration } from './host-update.js';
import type { SourceLineSnapshot } from './source-line-contracts.js';
import type { RegionNameChange } from './region-host-contracts.js';
import type { ClipboardObserver } from './native-clipboard.js';
import type { FeatureProfile } from '../../services/src/index.js';
import type { PromptMenuFactory } from './menu-presentation.js';

/** Mount initialization shares the atomic native configuration contract; callbacks observe rather than own editing state. */
export interface EditorOptions extends Omit<HostConfiguration, 'tagLexicon'> {
  readonly menuPresentation: PromptMenuFactory;
  readonly tooltipTransport: import('./tooltip-transport.js').TooltipTransportFactory;
  /** Source-free host recomputation ticket issued after edits or context retirement. */
  readonly onSearchRefresh?: (request: SearchRequest) => void;
  /** Host owns filter/reveal visibility; editor owns local highlight paint. */
  readonly onSearchIntent?: (intent: SearchHostIntent) => void;
  readonly source: string;
  readonly label: string;
  /** Literal mount hint; later string/empty intent uses the same native configuration owner. */
  readonly placeholder: string;
  readonly readOnly: boolean;
  readonly tagLexicon?: TagLexicon;
  /** Observe initial and ordinary native/host signals, including repeated values; certified internal presentation-only transactions may be silent when public inputs are unchanged. */
  readonly onUpdate: (snapshot: EditorSnapshot) => void;
  /** Observe accepted native context-menu raw/rich intent; hosts may persist this boolean. Programmatic host configuration is silent. */
  readonly onRichRenderingChange?: (enabled: boolean) => void;
  /** Acknowledge the actual adopted profile before native observations; reentrant neutral intent does not revoke accepted policy. */
  readonly onHostConfigurationAccepted?: (profile: FeatureProfile) => void;
  readonly onLayout?: (metrics: EditorLayoutMetrics) => void;
  readonly onSourceLines?: (snapshot: SourceLineSnapshot) => void;
  readonly onNestedParentheses?: (nestingDepth: number) => void;
  /** Unexpected decoder failure stays with the finite host reporting boundary. */
  readonly onLoraMediaError?: (cause: unknown) => void;
  /** Current unexpected dictionary failures retain their original cause at the mounted host boundary. */
  readonly onSpellcheckError?: (cause: unknown) => void;
  /** Authored regional transitions remain source-current; baselines and canonical peer loading emit no rename. */
  readonly onRegionNameChange?: (change: RegionNameChange) => void;
  /** Observe pointer hover of a prompt document's separator ordinal, and null when it ends; host-linked hover is silent. */
  readonly onRegionHover?: (ordinal: number | null) => void;
  /** Source-free expected clipboard outcomes and unexpected failures belong to the mounted host. */
  readonly clipboardObserver?: ClipboardObserver;
}
