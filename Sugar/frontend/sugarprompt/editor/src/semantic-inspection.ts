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
/** Explicit public semantic inspection consumes the same bounded native source owner without changing history. */
import { Transaction } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import { nativeCompositionStarted } from './input-settlement.js';
import { documentToSource, sourceToDocument } from '../../core/src/line-endings.js';
import type { WildcardSyntax } from '../../core/src/index.js';
import type {
  SceneContextInspection,
  StructureInspection,
  ConditioningInspection,
} from './contracts.js';
import { lineEndings } from './exact-source.js';
import { documentKind } from './document-kind.js';
import {
  installSemanticStructure,
  prepareInspectionStructure,
  semanticStructures,
  prepareNativeStructure,
} from './structure.js';
import {
  nativeConditioning,
  prepareConditioning,
  installConditioning,
} from './conditioning-state.js';

/** Explicit host inspection shares the semantic cache and prepared topology without adopting a custom inspection grammar. */
export function inspectConditioning(view: EditorView): ConditioningInspection {
  if (nativeCompositionStarted(view)) return { status: 'composing' };
  const kind = view.state.facet(documentKind);
  if (kind !== 'prompt') return { status: 'unsupported', documentKind: kind };
  let current = view.state.field(nativeConditioning);
  if (!current) {
    const semantic = prepareNativeStructure(view.state);
    current = prepareConditioning(view.state, semantic);
    view.dispatch({
      effects: [
        installConditioning.of(current),
        installSemanticStructure.of({ purpose: 'native', prepared: semantic }),
      ],
      annotations: Transaction.addToHistory.of(false),
    });
  }
  return {
    status: 'ready',
    context: current.context,
    topology: current.topology,
  };
}

/** Prepare and publish only missing current data; existing native and inspection outputs are reused. */
function inspect(view: EditorView, profile: WildcardSyntax) {
  const prepared = prepareInspectionStructure(view.state, profile);
  if (prepared !== view.state.field(semanticStructures).inspection)
    view.dispatch({
      effects: installSemanticStructure.of({ purpose: 'inspection', prepared }),
      annotations: Transaction.addToHistory.of(false),
    });
  return prepared;
}

/** Read-only grammar requests cannot interrupt unfinished native preedit or configure authoring grammar. */
export function inspectStructure(view: EditorView, profile: WildcardSyntax): StructureInspection {
  if (nativeCompositionStarted(view)) return { status: 'composing' };
  const prepared = inspect(view, profile);
  const shared = {
    status: 'ready' as const,
    scopes: prepared.scopes,
    documentSemantics: prepared.documentSemantics,
    loraResourceNames: prepared.loraScopes.resourceNames,
  };
  return prepared.kind === 'prompt'
    ? { ...shared, kind: 'prompt', structure: prepared.structure }
    : { ...shared, kind: prepared.kind };
}

/** Exact-source UTF-16 positions default to the native directional head; CRLF interiors and out-of-source bounds reject. */
export function inspectSceneContext(view: EditorView, position?: number): SceneContextInspection {
  if (nativeCompositionStarted(view)) return { status: 'composing' };
  const logical = view.state.doc.toString(),
    endings = view.state.field(lineEndings);
  const requested =
    position === undefined
      ? documentToSource(logical, endings, view.state.selection.main.head)
      : position;
  sourceToDocument(logical, endings, requested);
  const kind = view.state.facet(documentKind);
  if (kind !== 'prompt') return { status: 'unsupported', documentKind: kind };
  const inspected = inspect(view, { style: 'curly' });
  if (inspected.kind !== 'prompt') throw new Error('scene-inspection-kind-mismatch');
  const prepared = inspected.sceneContexts;
  return {
    status: 'ready',
    context: prepared.at(requested),
    effectivePromptTexts: prepared.effectivePromptTexts,
  };
}
