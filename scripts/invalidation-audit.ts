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
/** Enforce native observation ownership using resolved DOM types rather than spelling patterns. */
import ts from 'typescript';
import path from 'node:path';
import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

export interface InvalidationDiagnostic {
  readonly path: string;
  readonly line: number;
  readonly rule: string;
  readonly message: string;
}

/** Reject raw observers outside approved owners, including aliased native constructors. */
export function auditInvalidation(
  program: ts.Program,
  root: string,
  owners: ReadonlySet<string>,
  documentOwners: ReadonlySet<string> = new Set(),
): InvalidationDiagnostic[] {
  const checker = program.getTypeChecker();
  const failures: InvalidationDiagnostic[] = [];
  for (const source of program.getSourceFiles()) {
    const relative = path.relative(root, source.fileName).replaceAll('\\', '/');
    if (!relative.startsWith('frontend/') || source.isDeclarationFile) continue;
    const fail = (node: ts.Node, rule: string, message: string): void => {
      failures.push({
        path: relative,
        line: source.getLineAndCharacterOfPosition(node.getStart()).line + 1,
        rule,
        message,
      });
    };
    const visit = (node: ts.Node): void => {
      if (ts.isNewExpression(node)) {
        const signatures = checker
          .getNonNullableType(checker.getTypeAtLocation(node.expression))
          .getConstructSignatures();
        if (
          signatures.some((signature) =>
            isDomType(checker.getReturnTypeOfSignature(signature), 'MutationObserver'),
          ) &&
          !owners.has(relative)
        )
          fail(
            node,
            'UI_OBSERVER_OWNER',
            'Construct MutationObserver only in an approved native observation owner; use HostSurfaceObserver for surface lifecycle.',
          );
        const symbol = checker.getTypeAtLocation(node).getSymbol();
        if (symbol?.name === 'HostSurfaceObserver') {
          const selector = node.arguments?.[1];
          const scope = selector ? staticString(selector, checker, new Set()) : null;
          if (scope === null || /(^|,)\s*(?:\*|body|html)\s*(,|$)/i.test(scope))
            fail(
              node,
              'UI_SURFACE_SCOPE',
              'Declare a literal selector for identified native surfaces; document-wide wildcard discovery is forbidden.',
            );
        }
      }
      if (
        ts.isCallExpression(node) &&
        ts.isPropertyAccessExpression(node.expression) &&
        node.expression.name.text === 'observe' &&
        isDomType(
          checker.getNonNullableType(checker.getTypeAtLocation(node.expression.expression)),
          'MutationObserver',
        ) &&
        !owners.has(relative)
      )
        fail(
          node,
          'UI_OBSERVER_OWNER',
          'Observe native mutations only through the approved observation owner.',
        );
      if (
        ts.isCallExpression(node) &&
        ts.isPropertyAccessExpression(node.expression) &&
        node.expression.name.text === 'observe' &&
        isDomType(
          checker.getNonNullableType(checker.getTypeAtLocation(node.expression.expression)),
          'MutationObserver',
        ) &&
        node.arguments[0] &&
        isDocumentRoot(node.arguments[0], checker, new Set()) &&
        node.arguments[1] &&
        observesSubtree(node.arguments[1], checker) &&
        !documentOwners.has(relative)
      )
        fail(
          node,
          'UI_DOCUMENT_OBSERVER',
          'Document subtree observation belongs to an approved filtering adapter; observe the identified local surface instead.',
        );
      ts.forEachChild(node, visit);
    };
    visit(source);
  }
  return failures;
}

/** Distinguish local palette attributes from document-wide descendant observation. */
function observesSubtree(
  options: ts.Expression,
  checker: ts.TypeChecker,
  seen = new Set<ts.Node>(),
): boolean {
  if (seen.has(options)) return true;
  seen.add(options);
  if (ts.isObjectLiteralExpression(options)) {
    const property = options.properties.find(
      (candidate) => candidate.name?.getText() === 'subtree',
    );
    return (
      !!property &&
      !(
        ts.isPropertyAssignment(property) &&
        property.initializer.kind === ts.SyntaxKind.FalseKeyword
      )
    );
  }
  if (ts.isIdentifier(options)) {
    const declaration = checker.getSymbolAtLocation(options)?.valueDeclaration;
    if (
      declaration &&
      ts.isVariableDeclaration(declaration) &&
      declaration.initializer &&
      declaration.initializer !== options
    )
      return observesSubtree(declaration.initializer, checker, seen);
  }
  return checker.getTypeAtLocation(options).getProperty('subtree') !== undefined;
}

/** Follow native document-root aliases without mistaking arbitrary element properties for DOM roots. */
function isDocumentRoot(node: ts.Expression, checker: ts.TypeChecker, seen: Set<ts.Node>): boolean {
  if (seen.has(node)) return false;
  seen.add(node);
  if (
    ['Document', 'HTMLBodyElement', 'HTMLHtmlElement'].some((name) =>
      isDomType(checker.getNonNullableType(checker.getTypeAtLocation(node)), name),
    )
  )
    return true;
  if (ts.isPropertyAccessExpression(node) && ['body', 'documentElement'].includes(node.name.text))
    return (
      checker
        .getSymbolAtLocation(node.name)
        ?.declarations?.some(
          (declaration) => path.basename(declaration.getSourceFile().fileName) === 'lib.dom.d.ts',
        ) ?? false
    );
  if (ts.isIdentifier(node)) {
    const declaration = checker.getSymbolAtLocation(node)?.valueDeclaration;
    return !!(
      declaration &&
      ts.isVariableDeclaration(declaration) &&
      declaration.initializer &&
      isDocumentRoot(declaration.initializer, checker, seen)
    );
  }
  if (ts.isBinaryExpression(node))
    return (
      isDocumentRoot(node.left, checker, new Set(seen)) ||
      isDocumentRoot(node.right, checker, new Set(seen))
    );
  if (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isNonNullExpression(node))
    return isDocumentRoot(node.expression, checker, seen);
  return false;
}

/** Resolve declared selector constants without executing authored expressions. */
function staticString(
  node: ts.Expression,
  checker: ts.TypeChecker,
  seen: Set<ts.Node>,
): string | null {
  if (seen.has(node)) return null;
  seen.add(node);
  if (ts.isStringLiteralLike(node)) return node.text;
  if (ts.isIdentifier(node)) {
    const declaration = checker.getSymbolAtLocation(node)?.valueDeclaration;
    return declaration && ts.isVariableDeclaration(declaration) && declaration.initializer
      ? staticString(declaration.initializer, checker, seen)
      : null;
  }
  if (ts.isTemplateExpression(node)) {
    let value = node.head.text;
    for (const span of node.templateSpans) {
      const part = staticString(span.expression, checker, new Set(seen));
      if (part === null) return null;
      value += part + span.literal.text;
    }
    return value;
  }
  if (
    ts.isCallExpression(node) &&
    ts.isPropertyAccessExpression(node.expression) &&
    node.expression.name.text === 'join' &&
    ts.isArrayLiteralExpression(node.expression.expression)
  ) {
    const parts = node.expression.expression.elements.map((part) =>
      staticString(part, checker, new Set(seen)),
    );
    const separator = node.arguments[0]
      ? staticString(node.arguments[0], checker, new Set(seen))
      : ',';
    return parts.every((part): part is string => part !== null) && separator !== null
      ? parts.join(separator)
      : null;
  }
  return null;
}

/** Recognize the browser contract even when its constructor is imported through an alias. */
function isDomType(type: ts.Type, name: string): boolean {
  const symbol = type.getSymbol();
  return (
    (symbol?.name === name &&
      (symbol.declarations ?? []).some(
        (declaration) => path.basename(declaration.getSourceFile().fileName) === 'lib.dom.d.ts',
      )) ||
    (type.isClassOrInterface() && (type.getBaseTypes() ?? []).some((base) => isDomType(base, name)))
  );
}

/** Audit the exact source and owner inventory supplied by the architecture snapshot owner. */
function main(): void {
  const root = path.resolve(process.argv[2] ?? process.cwd());
  const input: unknown = JSON.parse(readFileSync(0, 'utf8'));
  if (
    !input ||
    typeof input !== 'object' ||
    !('files' in input) ||
    !('owners' in input) ||
    !('documentOwners' in input) ||
    !Array.isArray(input.files) ||
    !Array.isArray(input.owners) ||
    !Array.isArray(input.documentOwners) ||
    !input.files.every((value: unknown) => typeof value === 'string') ||
    !input.owners.every((value: unknown) => typeof value === 'string') ||
    !input.documentOwners.every((value: unknown) => typeof value === 'string')
  )
    throw new Error('Invalid observation policy inventory.');
  const files: string[] = input.files;
  const owners: string[] = input.owners;
  const documentOwners: string[] = input.documentOwners;
  const program = ts.createProgram(
    files.map((file) => path.join(root, file)),
    {
      target: ts.ScriptTarget.ES2022,
      module: ts.ModuleKind.NodeNext,
      moduleResolution: ts.ModuleResolutionKind.NodeNext,
      strict: true,
      skipLibCheck: true,
    },
  );
  process.stdout.write(
    JSON.stringify(auditInvalidation(program, root, new Set(owners), new Set(documentOwners))),
  );
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
