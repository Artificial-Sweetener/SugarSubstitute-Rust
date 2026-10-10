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
/** Present node master selectors and field-link choices on eligible native Cube cards. */
import type { WorkflowRelations } from '../application/WorkflowRelations.js';
import {
  canLinkField,
  canLinkNode,
  compatibleNodes,
  linkedFieldMembers,
} from '../domain/RelationResolver.js';
import { choiceRestrictions } from '../domain/FieldCompatibility.js';
import type { RelationWidgetRenderer, RelationChoiceMount } from './RelationWidgetRenderer.js';
import type { RelationCardSurface } from './RelationCardSurface.js';
import type { FieldContract, NodeContract } from '../domain/RelationContracts.js';
import { prepareNativePresentation } from '../../surface/NativePresentationReady.js';
import { setNativeFieldSuppressed } from '../../surface/NativeFieldPresentation.js';

export interface RelationCardView {
  readonly nodeId: string;
  readonly surface: RelationCardSurface;
}
interface MountedChoice {
  readonly choice: RelationChoiceMount;
  readonly root: HTMLElement;
}

/** Reconcile chrome independently from native card construction and compatibility policy. */
export class CubeRelationPresenter {
  readonly #choices = new Map<HTMLElement, Map<string, MountedChoice>>();
  #hidden = new Set<HTMLElement>();

  /** Receive identified native view boundaries through an explicit adapter. */
  constructor(
    private readonly document: Document,
    private readonly relations: WorkflowRelations,
    private readonly getViews: () => readonly RelationCardView[],
    private readonly reportError: (error: unknown) => void,
    private readonly widgets: RelationWidgetRenderer,
  ) {}

  /** Add selectors only where a compatible peer exists or an existing link needs display. */
  reconcile(): void {
    const hidden = new Set<HTMLElement>();
    const snapshot = this.relations.snapshot(),
      views = this.getViews(),
      live = new Set(views.map((view) => view.surface.root));
    for (const [root, choices] of this.#choices)
      if (!live.has(root) || !root.isConnected) {
        for (const mounted of choices.values()) this.#release(mounted);
        this.#choices.delete(root);
      }
    for (const view of views) {
      const node = snapshot.nodes.find((candidate) => candidate.id === view.nodeId);
      const fields = snapshot.fields.filter((field) => field.nodeId === view.nodeId);
      const mounted = this.#choices.get(view.surface.root) ?? new Map<string, MountedChoice>();
      this.#choices.set(view.surface.root, mounted);
      const retain = new Set<string>();
      if (node) this.#node(node, view.surface, mounted, retain);
      for (const field of fields) this.#field(field, view.surface, mounted, retain, hidden);
      for (const [key, choice] of mounted)
        if (!retain.has(key)) {
          this.#release(choice);
          mounted.delete(key);
        }
    }
    for (const element of this.#hidden)
      if (!hidden.has(element)) setNativeFieldSuppressed(element, false);
    for (const element of hidden)
      if (!this.#hidden.has(element)) setNativeFieldSuppressed(element, true);
    this.#hidden = hidden;
  }

  /** Restore native presentation and release all owned link controls. */
  dispose(): void {
    for (const choices of this.#choices.values())
      for (const mounted of choices.values()) this.#release(mounted);
    this.#choices.clear();
    for (const element of this.#hidden) setNativeFieldSuppressed(element, false);
    this.#hidden.clear();
  }

  /** Put compatible clone targets into the native titlebar's trailing flex row. */
  #node(
    node: NodeContract,
    surface: RelationCardSurface,
    mounted: Map<string, MountedChoice>,
    retain: Set<string>,
  ): void {
    const snapshot = this.relations.snapshot(),
      peers = snapshot.nodes.filter((peer) => compatibleNodes(node, peer));
    const earlier = peers.filter(
      (peer) => snapshot.nodes.indexOf(peer) < snapshot.nodes.indexOf(node),
    );
    const targets = earlier.filter((peer) =>
      canLinkNode(node, peer, snapshot.nodes, snapshot.state),
    );
    const source = snapshot.state.nodeLinks[node.id];
    if (!earlier.length && !source && !mounted.has('node')) return;
    const target = surface.header?.firstElementChild;
    if (!(target instanceof HTMLElement) && !surface.canvasTitle) return;
    const choice = this.#choice('node', mounted, `Link ${node.label}`, (value) =>
      this.relations.linkNode(node.id, value || null),
    );
    choice.root.classList.add('sc-node-link');
    const first = snapshot.nodes.find(
      (candidate) => candidate.id === node.id || peers.includes(candidate),
    );
    choice.root.hidden = first?.id === node.id;
    const options = [
      { value: '', label: 'Independent', blockers: [] },
      ...targets.map((peer) => ({
        value: peer.id,
        label: `🔗 ${peer.cubeLabel}`,
        blockers: [],
      })),
    ];
    choice.choice.update(source ?? '', options, false, [
      'Independent',
      ...[node, ...peers].map((candidate) => `🔗 ${candidate.cubeLabel}`),
    ]);
    if (target instanceof HTMLElement) {
      const activation = target.querySelector('.sugarcubes-cube-face__activation');
      if (
        choice.root.parentElement !== target ||
        (activation && choice.root.nextElementSibling !== activation)
      ) {
        target.insertBefore(choice.root, activation);
        prepareNativePresentation(choice.root);
      }
    } else {
      choice.root.classList.add('sc-relation-canvas-header');
      if (surface.canvas) surface.root.style.position = 'relative';
      if (choice.root.parentElement !== surface.root) {
        surface.root.append(choice.root);
        prepareNativePresentation(choice.root);
      }
    }
    retain.add('node');
  }

  /** Replace a native combo with its literal and link choices without storing a link label as a value. */
  #field(
    field: FieldContract,
    surface: RelationCardSurface,
    mounted: Map<string, MountedChoice>,
    retain: Set<string>,
    hidden: Set<HTMLElement>,
  ): void {
    const snapshot = this.relations.snapshot(),
      effective = snapshot.resolved.find((item) => item.field.id === field.id),
      row = surface.rows.get(field.name),
      canvasField = surface.canvasFields.get(field.name);
    if (!row && !canvasField) return;
    if (effective?.override) {
      if (row && !surface.nativeWidgetVisibility) hidden.add(row);
      return;
    }
    if (effective?.source && snapshot.state.nodeLinks[field.nodeId]) {
      if (row && !surface.nativeWidgetVisibility) hidden.add(row);
      return;
    }
    const peers = field.cubeId
      ? snapshot.fields.filter((peer) =>
          canLinkField(field, peer, snapshot.fields, snapshot.nodes, snapshot.state),
        )
      : [];
    const members = linkedFieldMembers(field.id, snapshot.fields, snapshot.nodes, snapshot.state);
    if (field.domain.kind !== 'choice' || (!peers.length && members.length < 2)) return;
    const key = field.id,
      source = snapshot.state.fieldLinks[field.id];
    const choice = this.#choice(
      key,
      mounted,
      `${field.name} link or value`,
      (value) => {
        if (value.startsWith('link:')) this.relations.linkField(field.id, value.slice(5));
        else {
          if (this.relations.snapshot().state.fieldLinks[field.id])
            this.relations.linkField(field.id, null);
          this.relations.setFieldValue(field.id, value);
        }
      },
      field.name,
    );
    const control = row
      ? (row.querySelector<HTMLElement>('[data-sc-fluent-combo-frame]') ??
        row
          .querySelector<HTMLElement>('[data-testid="widget-select-default-trigger"]')
          ?.closest<HTMLElement>('[data-capture-wheel]') ??
        row.lastElementChild)
      : null;
    if (control instanceof HTMLElement) {
      if (choice.root.parentElement !== control.parentElement) {
        control.parentElement?.append(choice.root);
        prepareNativePresentation(choice.root);
      }
      hidden.add(control);
    } else if (canvasField) {
      surface.root.style.position = surface.canvas ? 'relative' : surface.root.style.position;
      choice.root.classList.add('sc-relation-canvas-field');
      Object.assign(choice.root.style, {
        top: `${canvasField.top}px`,
        left: `${canvasField.left}px`,
        width: `${canvasField.width}px`,
        height: `${canvasField.height}px`,
      });
      if (choice.root.parentElement !== surface.root) {
        surface.root.append(choice.root);
        prepareNativePresentation(choice.root);
      }
    } else return;
    const restrictions = choiceRestrictions(members.length > 1 ? members : [field]);
    choice.choice.update(
      source ? `link:${source}` : String(effective?.value ?? field.value),
      [
        ...restrictions.map((item) => ({ ...item, label: item.value })),
        ...peers.map((peer) => ({
          value: `link:${peer.id}`,
          label: `🔗 ${peer.label}`,
          blockers: [],
        })),
      ],
      Boolean(effective?.source && !source),
    );
    retain.add(key);
  }

  /** Maintain stable choice identity through native redraws. */
  #choice(
    key: string,
    mounted: Map<string, MountedChoice>,
    label: string,
    operation: (value: string) => void,
    nativeInputName?: string,
  ): MountedChoice {
    let existing = mounted.get(key);
    if (!existing) {
      const root = this.document.createElement('div');
      root.className = 'sc-field-relation';
      const choice = this.widgets.mountChoice(
        root,
        label,
        (value) => {
          try {
            operation(value);
          } catch (error: unknown) {
            this.reportError(error);
          }
        },
        nativeInputName,
      );
      existing = { root, choice };
      mounted.set(key, existing);
    }
    return existing;
  }

  /** Release the link control while the visibility diff restores its native row. */
  #release(mounted: MountedChoice): void {
    mounted.choice.dispose();
    mounted.root.remove();
  }
}
