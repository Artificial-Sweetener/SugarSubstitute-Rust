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
/** List source locations where SugarCubes consumes native frontend structures or selectors. */
import ts from 'typescript';
import { relative } from 'node:path';
import { readFileSync } from 'node:fs';

interface Touch {
  path: string;
  line: number;
  kind: string;
  assumption: string;
  intent: string;
}

const options = ts.readConfigFile('tsconfig.json', (file) => readFileSync(file, 'utf8'));
const config = ts.parseJsonConfigFileContent(options.config, ts.sys, process.cwd());
const integrationFiles: string[] = JSON.parse(readFileSync(0, 'utf8'));
const program = ts.createProgram(
  [...new Set([...config.fileNames, ...integrationFiles])],
  config.options,
);
const checker = program.getTypeChecker();
const results: Touch[] = [];
const hostMember =
  /^(?:app(?:Ref|Record)?|comfyApp|litegraph|lgraphcanvas|lgraphnode|comfyAPI|graph|canvas|nodeType|nodeDef|nodeData|workflowStore|extensionManager|graphCanvas|promptQueue)$/i;
const hostTypes =
  /^(?:Comfy(?:App(?:lication)?|Graph|Group|Node(?:Definition|Constructor)?|Widget|Input|Output|Link|Canvas|HostApi)|LiteGraphHost|LGraph(?:Canvas|Node)?)$/;
const cssAtom =
  /(?:[#.](?:graph-canvas(?:-container)?|lg-[\w-]+|p-[\w-]+|comfy[\w-]*|graph-canvas-panel|actionbar-container|workflow-tabs-container|litecontextmenu|litegraph)|\[data-(?:testid|node-id|pc-name|p-)[^\]\n]*\]|--(?:comfy|p-|litegraph)[\w-]*)/g;
const hostLiteral =
  /^(?:\/scripts\/|\/api\/(?!sugarcubes)|\/(?:prompt|queue|object_info|history|view|upload|api|userdata|settings|embeddings)(?:[/?]|$)|Comfy\.|LiteGraph\.)/;

for (const source of program.getSourceFiles()) {
  const path = relative(process.cwd(), source.fileName).replaceAll('\\', '/');
  if (
    !path.startsWith('frontend/') &&
    !path.startsWith('integrations/') &&
    !path.startsWith('types/comfyui/')
  )
    continue;
  const intent = (item: ts.Node): string => {
    for (let scope: ts.Node | undefined = item; scope; scope = scope.parent) {
      for (const doc of ts.getJSDocCommentsAndTags(scope)) {
        if (ts.isJSDoc(doc) && typeof doc.comment === 'string' && doc.comment.trim())
          return doc.comment.trim();
      }
    }
    const moduleDoc = source.text.match(/\/\*\*([\s\S]*?)\*\//);
    return moduleDoc?.[1]?.replace(/^\s*\* ?/gm, '').trim() ?? '';
  };
  const record = (node: ts.Node, kind: string, assumption: string): void => {
    const line = source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1;
    results.push({ path, line, kind, assumption, intent: intent(node) });
  };
  const visit = (node: ts.Node): void => {
    if (ts.isPropertyAccessExpression(node) || ts.isElementAccessExpression(node)) {
      const receiver = node.expression;
      const type = checker.getNonNullableType(checker.getTypeAtLocation(receiver));
      const symbol = type.getSymbol()?.name ?? '';
      const receiverName = ts.isIdentifier(receiver)
        ? receiver.text
        : ts.isPropertyAccessExpression(receiver)
          ? receiver.name.text
          : '';
      const access = node.getText(source);
      const namedHostChain =
        /^(?:this\.(?:#?options\.)?)?(?:app(?:Ref|Record)?|comfyApp|[Ll]ite[Gg]raph|LGraphCanvas|comfyAPI)(?:\??\.[A-Za-z_$][\w$]*)+$/.test(
          access,
        );
      const target = receiver.getText(source);
      if (
        /^this\.#(?:workflow|api|app|canvas|graph)$/.test(target) &&
        path.includes('Comfy') &&
        ts.isPropertyAccessExpression(node)
      ) {
        record(node, 'host adapter member', target.slice(6) + '.' + node.name.text);
      }
      if (hostTypes.test(symbol)) {
        const key = ts.isPropertyAccessExpression(node)
          ? node.name.text
          : node.argumentExpression && ts.isStringLiteralLike(node.argumentExpression)
            ? node.argumentExpression.text
            : null;
        if (key) record(node, 'typed host member', symbol + '.' + key);
      }
      if (hostTypes.test(symbol) || hostMember.test(receiverName) || namedHostChain) {
        record(node, 'host member', node.getText(source));
      }
    }
    if (
      ts.isStringLiteralLike(node) ||
      ts.isTemplateHead(node) ||
      ts.isTemplateMiddle(node) ||
      ts.isTemplateTail(node)
    ) {
      if (hostLiteral.test(node.text)) record(node, 'module, endpoint or setting', node.text);
      for (const atom of new Set(node.text.match(cssAtom)))
        record(node, 'host selector or token', atom);
    }
    if (ts.isCallExpression(node) && ts.isPropertyAccessExpression(node.expression)) {
      const method = node.expression.name.text;
      const argument = node.arguments[0];
      if (
        argument &&
        ['querySelector', 'querySelectorAll', 'getElementById', 'closest', 'matches'].includes(
          method,
        )
      ) {
        const selector = argument.getText(source);
        if (!/^[`'"](?:[.#](?:sc-|sugarcubes-)|\[data-sc-)/.test(selector))
          record(argument, 'DOM lookup', selector);
      }
      if (
        argument &&
        ['fetchApi', 'addEventListener', 'removeEventListener', 'dispatchCustomEvent'].includes(
          method,
        ) &&
        /(?:api|promptServer)$/i.test(node.expression.expression.getText(source)) &&
        ts.isStringLiteralLike(argument)
      ) {
        record(node, 'host transport', method + ' ' + argument.text);
      }
      if (
        node.expression.expression.getText(source) === 'Reflect' &&
        node.arguments[1] &&
        ts.isStringLiteral(node.arguments[1])
      ) {
        record(node, 'dynamic member', node.arguments[1].text);
      }
    }
    if (
      source.fileName.endsWith('ComfyRuntimeModuleLoader.ts') &&
      ts.isVariableDeclaration(node) &&
      node.initializer &&
      (ts.isRegularExpressionLiteral(node.initializer) || ts.isStringLiteral(node.initializer))
    ) {
      record(node, 'bundle discovery', node.initializer.getText(source));
    }
    if (ts.isPropertyAssignment(node) && /^Widget[A-Z]/.test(node.name.getText(source)))
      record(node, 'native component', node.name.getText(source));
    if (ts.isPropertyAssignment(node) || ts.isMethodDeclaration(node)) {
      const key = node.name.getText(source);
      if (
        /^(?:registerCustomNodes|addCustomNodeDefs|beforeRegisterNodeDef|beforeRegisterVueAppNodeDefs|nodeCreated|loadedGraphNode|beforeConfigureGraph|afterConfigureGraph|getCanvasMenuItems|getNodeMenuItems|refreshComboInNodes|setup)$/.test(
          key,
        ) &&
        path.startsWith('frontend/comfyui/') &&
        source.text.includes('registerExtension')
      )
        record(node, 'extension hook', key);
    }
    ts.forEachChild(node, visit);
  };
  visit(source);
}
process.stdout.write(JSON.stringify(results));
