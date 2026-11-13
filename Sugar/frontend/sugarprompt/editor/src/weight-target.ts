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
/** Source-backed numeric targets distinguish emphasis content, LoRA weights and integer wildcard groups. */
import type { EditorState } from '@codemirror/state';
import type { SourceRange } from '../../core/src/index.js';
import { featureEnabled } from '../../services/src/index.js';
import { featureProfile } from './features.js';

/** Logical-document UTF-16 bounds contain emphasis content or complete LoRA/wildcard outer syntax. */
export interface WeightSubject {
  readonly kind: 'emphasis' | 'lora' | 'wildcard';
  readonly range: SourceRange;
}

/** Decimal weight labels support exact drafts as well as step controls. */
export const weightLabelSelector = '.sp-emphasis-weight, .sp-lora-weight';
/** Numeric wildcard groups share step controls; exact decimal drafts keep the narrower weight selector. */
export const steppableLabelSelector = `${weightLabelSelector}, .sp-wildcard-tag[data-numeric='true']`;

/** One permission owner admits the specified semantic kind, or any displayed weighted kind for dormant observation. */
export function weightEditingAllowed(state: EditorState, kind?: WeightSubject['kind']): boolean {
  if (state.readOnly) return false;
  const profile = state.facet(featureProfile);
  return kind === undefined
    ? featureEnabled(profile, 'emphasis') ||
        featureEnabled(profile, 'lora_syntax') ||
        featureEnabled(profile, 'wildcard_syntax')
    : featureEnabled(
        profile,
        kind === 'emphasis' ? 'emphasis' : kind === 'lora' ? 'lora_syntax' : 'wildcard_syntax',
      );
}
