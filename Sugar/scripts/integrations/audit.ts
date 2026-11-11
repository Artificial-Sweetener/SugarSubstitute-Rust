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
/** Enforce editor ownership using compiler-resolved imports, including host module access. */
import path from 'node:path';
import ts from 'typescript';

/** Describe one registered editor and its native API boundary. */
export interface EditorBoundary {
  readonly name: string;
  readonly root: string;
  readonly infrastructure: string;
  readonly modules: readonly string[];
}

/** Describe shared roots and every registered native editor boundary. */
export interface IntegrationBoundaries {
  readonly contracts: string;
  readonly shared: string;
  readonly editors: readonly EditorBoundary[];
}

/** Return actionable ownership failures without evaluating integration code. */
export function auditIntegrations(
  root: string,
  files: readonly string[],
  policy: IntegrationBoundaries,
): string[] {
  const configFile = ts.readConfigFile(path.join(root, 'tsconfig.json'), ts.sys.readFile);
  if (configFile.error)
    throw new Error(ts.flattenDiagnosticMessageText(configFile.error.messageText, '\n'));
  const config = ts.parseJsonConfigFileContent(configFile.config, ts.sys, root);
  const failures: string[] = [];
  const inside = (file: string, directory: string): boolean => file.startsWith(`${directory}/`);
  const owner = (file: string): EditorBoundary | undefined =>
    policy.editors.find((editor) => inside(file, editor.root));
  for (const file of files) {
    const absolute = path.join(root, file);
    const source = ts.createSourceFile(
      absolute,
      ts.sys.readFile(absolute) ?? '',
      ts.ScriptTarget.Latest,
      true,
    );
    const editor = owner(file);
    const portable = inside(file, policy.contracts) || inside(file, policy.shared);
    const integration = inside(file, 'integrations');
    const native = editor && inside(file, editor.infrastructure);
    const fail = (node: ts.Node, message: string): void => {
      failures.push(
        `${file}:${source.getLineAndCharacterOfPosition(node.getStart()).line + 1}: ${message}`,
      );
    };
    if (integration && !portable && !editor)
      fail(source, 'Register the editor before adding integration code.');
    const dependency = (node: ts.Node, module: ts.Expression | ts.TypeNode): void => {
      const value = ts.isLiteralTypeNode(module) ? module.literal : module;
      if (!ts.isStringLiteralLike(value)) {
        if (integration) fail(node, 'Integration imports must name a static module.');
        return;
      }
      const specifier = value.text;
      const resolved = ts.resolveModuleName(specifier, absolute, config.options, ts.sys)
        .resolvedModule?.resolvedFileName;
      const nativeOwner = policy.editors.find((item) =>
        item.modules.some(
          (name) =>
            specifier === name ||
            specifier.startsWith(`${name}/`) ||
            resolved?.replaceAll('\\', '/').includes(`/node_modules/@types/${name}/`),
        ),
      );
      if (nativeOwner && (!native || nativeOwner.name !== editor?.name))
        fail(node, `Native ${nativeOwner.name} APIs belong to ${nativeOwner.infrastructure}.`);
      const target = resolved
        ? path.relative(root, resolved).replaceAll('\\', '/')
        : specifier.startsWith('.')
          ? path.posix.normalize(path.posix.join(path.posix.dirname(file), specifier))
          : specifier;
      const targetEditor = owner(target);
      if (
        editor &&
        (inside(file, `${editor.root}/src/domain`) ||
          inside(file, `${editor.root}/src/application`)) &&
        (inside(target, `${editor.root}/src/infrastructure`) ||
          inside(target, `${editor.root}/src/presentation`))
      )
        fail(
          node,
          'Domain and application code must receive typed ports instead of concrete host adapters.',
        );
      if (targetEditor && targetEditor.name !== editor?.name)
        fail(
          node,
          `Editor implementation ${targetEditor.name} cannot be imported across host boundaries.`,
        );
      if (integration && (target.startsWith('frontend/') || specifier.startsWith('/scripts/')))
        fail(
          node,
          'Integration code must use portable contracts instead of ComfyUI implementation imports.',
        );
      if (
        portable &&
        !inside(target, policy.contracts) &&
        !(inside(file, policy.shared) && inside(target, policy.shared))
      )
        fail(node, 'Portable code may depend only on contracts and its permitted shared layer.');
    };
    const visit = (node: ts.Node): void => {
      if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier)
        dependency(node, node.moduleSpecifier);
      if (
        ts.isImportEqualsDeclaration(node) &&
        ts.isExternalModuleReference(node.moduleReference) &&
        node.moduleReference.expression
      )
        dependency(node, node.moduleReference.expression);
      if (ts.isImportTypeNode(node)) dependency(node, node.argument);
      if (
        ts.isCallExpression(node) &&
        (node.expression.kind === ts.SyntaxKind.ImportKeyword ||
          (ts.isIdentifier(node.expression) && node.expression.text === 'require'))
      ) {
        const argument = node.arguments[0];
        if (argument) dependency(node, argument);
        else if (integration) fail(node, 'Module loading requires a static module name.');
      }
      if (
        integration &&
        ts.isIdentifier(node) &&
        node.text === 'require' &&
        !(ts.isCallExpression(node.parent) && node.parent.expression === node && native)
      )
        fail(node, 'Require may only be called directly inside a native infrastructure owner.');
      ts.forEachChild(node, visit);
    };
    visit(source);
  }
  return failures;
}
