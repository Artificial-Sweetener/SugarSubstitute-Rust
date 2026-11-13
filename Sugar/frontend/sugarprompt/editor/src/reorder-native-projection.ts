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
/** A finite view owner admits genuine native pickup targets in READ without replacing their rich paint. */
import type { Text } from '@codemirror/state';
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import type { DecorationSet } from '@codemirror/view';
import { reorderPreview } from './reorder-preview.js';
import { readReorderPaintProperty } from './reorder-paint-style.js';
import type { ReorderPreview } from './reorder-preview-contracts.js';
import type { MountedReorderTarget } from './reorder-mounted-target.js';
import { admitReorderLineLayout } from './reorder-layout-admission.js';
import { nativeReorderRecipe, nativeReorderElement } from './reorder-native-recipe.js';
import { nativeReorderTargetAttributes } from './reorder-native-targets.js';

/** Metadata borrows the exact native mark/widget declarations rather than repeating Core layout rules. */
type NativeTarget = Pick<MountedReorderTarget, 'kind' | 'index' | 'blank'>;
/** One declaration can have several actual native fragments across hard lines. */
interface TargetDeclaration {
  readonly target: NativeTarget;
  readonly from: number;
  readonly to: number;
  readonly fragments: { readonly from: number; readonly to: number }[];
}
/** Only one admitted native tree and its observer can belong to this finite view. */
interface NativeIssue {
  readonly paint: ReorderPreview;
  readonly decorations: DecorationSet;
  readonly hold: object;
  readonly targets: readonly MountedReorderTarget[];
  readonly lines: readonly HTMLElement[];
  readonly opacity: ReadonlyMap<HTMLElement, string>;
  observer: MutationObserver | null;
}
const owners = new WeakMap<EditorView, NativeReorderProjection>();

/** A DOM attribute reader adapts both issued declarations and actual mounted roots. */
function targetMetadata(read: (name: string) => string | null): NativeTarget | null {
  const chip = read('data-reorder-chip'),
    gap = read('data-reorder-gap'),
    partition = read('data-reorder-partition');
  const kind = chip !== null ? 'chip' : partition !== null ? 'partition' : 'gap';
  const raw = chip ?? partition ?? gap;
  if (raw === null || !/^(0|[1-9][0-9]*)$/.test(raw)) return null;
  const index = Number(raw),
    blank = read('data-reorder-blank');
  if (!Number.isSafeInteger(index)) return null;
  if (blank !== null && (!/^(0|[1-9][0-9]*)$/.test(blank) || !Number.isSafeInteger(Number(blank))))
    return null;
  return { kind, index, ...(blank !== null ? { blank: Number(blank) } : {}) };
}
/** Exact typed target identity distinguishes empty carrier slots from chip fragments. */
function targetKey(target: NativeTarget): string {
  return `${target.kind}:${target.index}:${target.blank ?? ''}`;
}
/** Native hard-line breaks have no text node; only actual CM line separators and empty lines can bridge root intervals. */
function nativeBreakGap(document: Text, from: number, to: number): boolean {
  let line = document.lineAt(from);
  if (line.to !== from) return false;
  for (let count = 0; count < 16 && line.number < document.lines; count++) {
    line = document.line(line.number + 1);
    if (line.from === to) return true;
    if (line.length || line.from > to) return false;
  }
  return false;
}
/** All native fragments must collectively witness the issued source interval, with UTF-16 logical-document offsets. */
function completeDeclaration(declaration: TargetDeclaration, document: Text): boolean {
  let end = declaration.from;
  for (const fragment of declaration.fragments.sort((a, b) => a.from - b.from)) {
    if (fragment.from > end && !nativeBreakGap(document, end, fragment.from)) return false;
    end = Math.max(end, fragment.to);
  }
  return (
    declaration.fragments.length > 0 &&
    (end === declaration.to || nativeBreakGap(document, end, declaration.to))
  );
}

/** ViewPlugin lifetime owns native target admission, invalidation and observer disposal. */
export class NativeReorderProjection {
  private readonly view: EditorView;
  private issue: NativeIssue | null = null;
  private disposed = false;
  private generation = 0;
  /** Registration lends only this exact live view owner; no global native tree survives teardown. */
  constructor(view: EditorView) {
    this.view = view;
    owners.set(view, this);
  }
  /** CodeMirror source, viewport and geometry updates retire actual target identity before a later READ can re-admit it. */
  update(update: ViewUpdate): void {
    if (
      update.docChanged ||
      update.viewportChanged ||
      update.geometryChanged ||
      (this.issue && !this.matches(this.issue))
    )
      this.retire();
  }
  /** Exact publication/context keys fence every borrow without traversing the native tree. */
  private matches(issue: NativeIssue): boolean {
    const publication = this.view.state.field(reorderPreview, false);
    return (
      !this.disposed &&
      owners.get(this.view) === this &&
      publication?.native === true &&
      publication.paint === issue.paint &&
      publication.decorations === issue.decorations &&
      publication.pointer?.mode === 'stage' &&
      publication.pointer.hold === issue.hold
    );
  }
  /** Pending native mutation records reject stale roots synchronously before any publication can borrow them. */
  private current(issue: NativeIssue): boolean {
    if (this.issue !== issue || !this.matches(issue)) return false;
    if (issue.observer?.takeRecords().length) {
      this.retire(issue);
      return false;
    }
    return (
      this.issue === issue &&
      this.matches(issue) &&
      issue.lines.every((line) => line.isConnected) &&
      this.issue === issue &&
      this.matches(issue) &&
      this.issue === issue
    );
  }
  /** Currentness access is scan-free; only an explicit READ admission can discover native targets. */
  targets(): readonly MountedReorderTarget[] | null {
    return this.issue && this.current(this.issue) ? this.issue.targets : null;
  }
  /** Capture only this already-observed target issue; callers cannot re-admit after a dirty topology snapshot. */
  layoutLease(targets: readonly MountedReorderTarget[]): (() => boolean) | null {
    const issue = this.issue;
    if (!issue || issue.targets !== targets) return null;
    return () => {
      if (!this.current(issue)) return false;
      // Connectivity access is delegated native work too; drain its mutations before lending copied topology.
      if (issue.observer?.takeRecords().length) {
        this.retire(issue);
        return false;
      }
      return this.issue === issue && this.matches(issue);
    };
  }
  /** Exact original line opacity survives whole-coverage CSS only under the still-current native issue. */
  lineOpacity(line: HTMLElement): string | null {
    const issue = this.issue;
    if (!issue || !this.current(issue)) return null;
    const opacity = issue.opacity.get(line);
    return this.issue === issue && this.current(issue) ? (opacity ?? null) : null;
  }
  /** Complete native marks, empty targets and source intervals are admitted once in the caller's supported CM READ. */
  admit(preview: ReorderPreview): (() => boolean) | null {
    const publication = this.view.state.field(reorderPreview, false);
    if (
      !publication?.native ||
      publication.value !== preview ||
      publication.pointer?.mode !== 'stage' ||
      !publication.paint ||
      this.disposed
    )
      return null;
    let issue = this.issue;
    const previousGeneration = this.generation;
    if (issue && !this.current(issue)) {
      if (this.issue && this.issue !== issue) return null;
      this.retire(issue);
      issue = null;
    }
    if (this.issue && this.issue !== issue) return null;
    if (issue && previousGeneration !== this.generation) return null;
    if (!issue) {
      if (publication.mask?.coverage === 'value') return null;
      const generation = ++this.generation;
      /** External DOM accessors cannot overwrite a synchronously admitted newer native issue. */
      const admissionCurrent = (): boolean =>
        !this.disposed &&
        owners.get(this.view) === this &&
        this.generation === generation &&
        this.view.state.field(reorderPreview, false) === publication;
      const lines = Array.from(this.view.contentDOM.querySelectorAll<HTMLElement>('.cm-line'));
      if (!admissionCurrent() || !lines.length || lines.length > 16) return null;
      const nodes = admitReorderLineLayout(lines);
      if (!admissionCurrent() || !nodes) return null;
      const declarations = new Map<string, TargetDeclaration>();
      publication.decorations.between(0, this.view.state.doc.length, (from, to, decoration) => {
        const attributes = nativeReorderTargetAttributes(decoration);
        const target = attributes && targetMetadata((name) => attributes[name] ?? null);
        if (target)
          declarations.set(targetKey(target), {
            target,
            from,
            to,
            fragments: [],
          });
      });
      if (!declarations.size || declarations.size > 8192) return null;
      const recipe = nativeReorderRecipe(
        preview.capture.context.values,
        preview.capture.target.entry,
      );
      if (!recipe) return null;
      const targets: MountedReorderTarget[] = [];
      for (const node of nodes) {
        if (node.nodeType !== 1) continue;
        const element = node as HTMLElement,
          target = targetMetadata((name) => element.getAttribute(name));
        if (!admissionCurrent()) return null;
        if (!target || !nativeReorderElement(element, recipe.key)) continue;
        const declaration = declarations.get(targetKey(target));
        if (!declaration || !element.isConnected) return null;
        const from = this.view.posAtDOM(element, 0),
          to = this.view.posAtDOM(element, element.childNodes.length);
        if (!admissionCurrent()) return null;
        if (
          !Number.isSafeInteger(from) ||
          !Number.isSafeInteger(to) ||
          from < declaration.from ||
          to > declaration.to ||
          from > to
        )
          return null;
        declaration.fragments.push({ from, to });
        targets.push(
          Object.freeze({
            node: element,
            ...target,
            leases: Object.freeze([]),
            origins: Object.freeze([]),
          }),
        );
      }
      if (
        ![...declarations.values()].every((declaration) =>
          completeDeclaration(declaration, this.view.state.doc),
        )
      )
        return null;
      const window = this.view.dom.ownerDocument.defaultView;
      if (!window || !admissionCurrent()) return null;
      const opacity = new Map<HTMLElement, string>();
      for (const line of lines) {
        opacity.set(
          line,
          readReorderPaintProperty(window.getComputedStyle(line), 'opacity') || '1',
        );
        if (!admissionCurrent()) return null;
      }
      const Observer = this.view.dom.ownerDocument.defaultView?.MutationObserver;
      if (!admissionCurrent() || !Observer) return null;
      const admitted: NativeIssue = {
        paint: publication.paint,
        decorations: publication.decorations,
        hold: publication.pointer.hold,
        targets: Object.freeze(targets),
        lines: Object.freeze(lines),
        opacity,
        observer: null,
      };
      try {
        admitted.observer = new Observer(() => {
          if (this.issue === admitted) this.retire(admitted);
        });
        admitted.observer.observe(this.view.contentDOM, {
          childList: true,
          subtree: true,
          characterData: true,
          attributes: true,
        });
        if (!admissionCurrent()) {
          const observer = admitted.observer;
          admitted.observer = null;
          observer.disconnect();
          return null;
        }
        this.issue = admitted;
        if (!this.current(admitted)) {
          this.retire(admitted);
          return null;
        }
      } catch (cause) {
        try {
          if (this.issue === admitted) this.retire(admitted);
          else {
            const observer = admitted.observer;
            admitted.observer = null;
            observer?.disconnect();
          }
        } catch (cleanup) {
          throw new AggregateError([cause, cleanup], 'Native projection admission cleanup failed');
        }
        throw cause;
      }
      issue = admitted;
    }
    const admitted = issue;
    return () =>
      this.view.state.field(reorderPreview, false)?.value === preview &&
      this.current(admitted) &&
      this.view.state.field(reorderPreview, false)?.value === preview;
  }
  /** Clear native identity before releasing its one observer, preserving unexpected raw native failures. */
  private retire(expected?: NativeIssue): void {
    if (expected && this.issue !== expected) return;
    ++this.generation;
    const issue = this.issue;
    this.issue = null;
    const observer = issue?.observer;
    if (issue) issue.observer = null;
    observer?.disconnect();
  }
  /** Native teardown cannot leave registered targets or observers reachable through the view registry. */
  destroy(): void {
    this.disposed = true;
    if (owners.get(this.view) === this) owners.delete(this.view);
    this.retire();
  }
}
/** Explicit READ admission has one finite installed native owner. */
export function admitNativeReorderProjection(
  view: EditorView,
  preview: ReorderPreview,
): (() => boolean) | null {
  return owners.get(view)?.admit(preview) ?? null;
}
/** Borrow only already-admitted actual native roots; WRITE/currentness never discover DOM. */
export function currentNativeReorderProjectionTargets(
  view: EditorView,
): readonly MountedReorderTarget[] | null {
  return owners.get(view)?.targets() ?? null;
}
/** Request-local layout reuse borrows the existing native mutation owner and exact target-array identity. */
export function borrowNativeReorderLayoutLease(
  view: EditorView,
  targets: readonly MountedReorderTarget[],
): (() => boolean) | null {
  return owners.get(view)?.layoutLease(targets) ?? null;
}
/** Composition installs the owner beside the preview state, avoiding a reverse dependency from the state field. */
export const nativeReorderProjectionExtension = ViewPlugin.fromClass(NativeReorderProjection);

/** Only the exact admitted native issue can lend its pre-coverage containing-line opacity. */
export function currentNativeReorderLineOpacity(
  view: EditorView,
  line: HTMLElement,
): string | null {
  return owners.get(view)?.lineOpacity(line) ?? null;
}
