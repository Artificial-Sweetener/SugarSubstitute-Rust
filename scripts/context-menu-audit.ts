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
/** Enforce shared context presentation through resolved constructors, factories and DOM semantics. */
import path from 'node:path';
import ts from 'typescript';
import type { InvalidationDiagnostic } from './invalidation-audit.js';

export interface MenuOwnership {
  readonly context_menu: string;
  readonly floating_surface: string;
  readonly fluent_runtime: string;
  readonly search_dialogs: readonly string[];
  readonly choice_controls: readonly string[];
}

/** Validate the menu policy at the compiler invocation boundary. */
export function isMenuOwnership(value: unknown): value is MenuOwnership {
  return (
    !!value &&
    typeof value === 'object' &&
    'context_menu' in value &&
    typeof value.context_menu === 'string' &&
    'floating_surface' in value &&
    typeof value.floating_surface === 'string' &&
    'fluent_runtime' in value &&
    typeof value.fluent_runtime === 'string' &&
    'search_dialogs' in value &&
    Array.isArray(value.search_dialogs) &&
    value.search_dialogs.every((owner: unknown) => typeof owner === 'string') &&
    'choice_controls' in value &&
    Array.isArray(value.choice_controls) &&
    value.choice_controls.every((owner: unknown) => typeof owner === 'string')
  );
}

/** Reject independent context renderers while allowing feature-owned entries and event delegation. */
export function auditContextMenus(
  program: ts.Program,
  root: string,
  policy: MenuOwnership,
): InvalidationDiagnostic[] {
  const checker = program.getTypeChecker();
  const failures: InvalidationDiagnostic[] = [];
  const rowOwners = new Set([policy.context_menu, ...policy.search_dialogs]);
  const popupOwners = new Set([
    policy.context_menu,
    ...policy.search_dialogs,
    ...policy.choice_controls,
  ]);
  for (const source of program.getSourceFiles()) {
    const relative = relativePath(root, source.fileName);
    if (!relative.startsWith('frontend/') || source.isDeclarationFile) continue;
    /** Report the owning boundary rather than prescribing feature-specific handlers. */
    const fail = (node: ts.Node, rule: string, message: string): void => {
      failures.push({
        path: relative,
        line: source.getLineAndCharacterOfPosition(node.getStart()).line + 1,
        rule,
        message,
      });
    };
    /** Keep menu roles inside the shared presenter and explicitly declared search result dialogs. */
    const role = (node: ts.Node, value: ts.Expression): void => {
      if (
        !rowOwners.has(relative) &&
        strings(value, checker).some((item) => /^menu(?:item(?:checkbox|radio)?)?$/.test(item))
      )
        fail(
          node,
          'UI_CONTEXT_MENU_RENDERER',
          'Render context menu rows through ContextMenu; features supply entries and actions.',
        );
    };
    /** Inspect resolved types and literal DOM semantics, including imported aliases. */
    const visit = (node: ts.Node): void => {
      if (ts.isNewExpression(node)) {
        const type = checker.getTypeAtLocation(node);
        const symbol = type.getSymbol();
        const names = [symbol?.name, constructorName(node.expression, checker, new Set())];
        if (names.includes('ContextMenu') && !declaredAt(symbol, root, policy.context_menu))
          fail(
            node,
            'UI_CONTEXT_MENU_OWNER',
            'Construct the shared SugarCubes ContextMenu instead of a host or feature-owned menu.',
          );
        if (
          names.includes('FloatingSurface') &&
          (!popupOwners.has(relative) || !declaredAt(symbol, root, policy.floating_surface))
        )
          fail(node, 'UI_CONTEXT_MENU_OWNER', 'Context popup lifecycle belongs to ContextMenu.');
      }
      if (
        ts.isHeritageClause(node) &&
        node.token === ts.SyntaxKind.ExtendsKeyword &&
        relative !== policy.context_menu
      )
        for (const base of node.types)
          if (checker.getTypeAtLocation(base).getSymbol()?.name === 'ContextMenu')
            fail(
              base,
              'UI_CONTEXT_MENU_OWNER',
              'Compose ContextMenu entries rather than subclassing its interaction owner.',
            );
          else if (
            checker.getTypeAtLocation(base).getSymbol()?.name === 'FloatingSurface' &&
            relative !== policy.floating_surface
          )
            fail(
              base,
              'UI_POPUP_OWNER',
              'Compose the shared floating owner instead of subclassing popup transport.',
            );
      if (
        ts.isPropertyAccessExpression(node) &&
        ['showPopover', 'hidePopover'].includes(node.name.text) &&
        relative !== policy.floating_surface
      ) {
        const symbol = checker.getSymbolAtLocation(node.name);
        if (
          symbol?.declarations?.some(
            (declaration) => path.basename(declaration.getSourceFile().fileName) === 'lib.dom.d.ts',
          )
        )
          fail(
            node,
            'UI_POPUP_OWNER',
            'Use the shared floating owner for browser popover lifecycle.',
          );
      }
      if (ts.isCallExpression(node)) {
        const signature = checker.getResolvedSignature(node)?.declaration;
        if (
          signature &&
          ts.isFunctionDeclaration(signature) &&
          ['createFluentMenuList', 'createFluentMenuItem'].includes(signature.name?.text ?? '') &&
          relativePath(root, signature.getSourceFile().fileName) === policy.fluent_runtime &&
          !rowOwners.has(relative) &&
          relative !== policy.fluent_runtime
        )
          fail(
            node,
            'UI_CONTEXT_MENU_RENDERER',
            'Use ContextMenu instead of constructing independent Fluent menu rows.',
          );
        const resolvedName =
          signature &&
          (ts.isMethodSignature(signature) ||
            ts.isMethodDeclaration(signature) ||
            ts.isFunctionDeclaration(signature)) &&
          signature.name
            ? propertyName(signature.name)
            : null;
        const callee =
          resolvedName ??
          (ts.isPropertyAccessExpression(node.expression)
            ? node.expression.name.text
            : node.expression.getText(source));
        if (callee === 'setAttribute') {
          const attribute = node.arguments[0];
          const value = node.arguments[1];
          if (attribute && value) {
            const names = strings(attribute, checker);
            if (names.includes('role')) role(node, value);
            if (names.includes('popover') && relative !== policy.floating_surface)
              fail(
                node,
                'UI_POPUP_OWNER',
                'Declare browser popovers only in the shared floating owner.',
              );
          }
        }
        if (
          ['createElement', 'h', 'createVNode', 'jsx', 'jsxs', 'jsxDEV'].includes(callee) &&
          node.arguments[0] &&
          strings(node.arguments[0], checker).some((tag) =>
            ['menu', 'fluent-menu-list', 'fluent-menu-item'].includes(tag),
          ) &&
          relative !== policy.fluent_runtime &&
          !rowOwners.has(relative)
        )
          fail(
            node,
            'UI_CONTEXT_MENU_RENDERER',
            'Construct menu content through the shared ContextMenu.',
          );
      }
      if (ts.isPropertyAssignment(node) && propertyName(node.name) === 'role')
        role(node, node.initializer);
      if (
        ts.isPropertyAssignment(node) &&
        propertyName(node.name) === 'popover' &&
        relative !== policy.floating_surface
      )
        fail(node, 'UI_POPUP_OWNER', 'Declare browser popovers only in the shared floating owner.');
      if (
        ts.isBinaryExpression(node) &&
        node.operatorToken.kind === ts.SyntaxKind.EqualsToken &&
        ts.isPropertyAccessExpression(node.left) &&
        node.left.name.text === 'role'
      )
        role(node, node.right);
      if (ts.isJsxAttribute(node) && node.name.getText(source) === 'role' && node.initializer) {
        const value = ts.isJsxExpression(node.initializer)
          ? node.initializer.expression
          : node.initializer;
        if (value) role(node, value);
      }
      if (
        ts.isJsxAttribute(node) &&
        node.name.getText(source) === 'popover' &&
        relative !== policy.floating_surface
      )
        fail(node, 'UI_POPUP_OWNER', 'Declare browser popovers only in the shared floating owner.');
      ts.forEachChild(node, visit);
    };
    visit(source);
  }
  return failures;
}

/** Keep owner identity exact even in a materialized Git index snapshot. */
function relativePath(root: string, file: string): string {
  return path.relative(root, file).replaceAll('\\', '/');
}

/** Resolve compile-time strings through constants and literal unions. */
function strings(node: ts.Expression, checker: ts.TypeChecker): readonly string[] {
  if (ts.isStringLiteralLike(node)) return [node.text];
  const type = checker.getTypeAtLocation(node);
  const types = type.isUnion() ? type.types : [type];
  return types.flatMap((value) => (value.isStringLiteral() ? [value.value] : []));
}

/** Recognize an exact canonical class declaration rather than accepting a matching class name. */
function declaredAt(symbol: ts.Symbol | undefined, root: string, owner: string): boolean {
  return (
    symbol?.declarations?.some(
      (declaration) => relativePath(root, declaration.getSourceFile().fileName) === owner,
    ) ?? false
  );
}

/** Trace host constructor aliases even when the external boundary has no usable return type. */
function constructorName(
  node: ts.Expression,
  checker: ts.TypeChecker,
  seen: Set<ts.Node>,
): string | null {
  if (seen.has(node)) return null;
  seen.add(node);
  if (ts.isPropertyAccessExpression(node)) return node.name.text;
  const declaration = checker.getSymbolAtLocation(node)?.valueDeclaration;
  if (declaration && ts.isVariableDeclaration(declaration) && declaration.initializer)
    return constructorName(declaration.initializer, checker, seen);
  return ts.isIdentifier(node) ? node.text : null;
}

/** Admit literal property names without evaluating authored code. */
function propertyName(node: ts.PropertyName): string | null {
  return ts.isIdentifier(node) || ts.isStringLiteralLike(node) ? node.text : null;
}
