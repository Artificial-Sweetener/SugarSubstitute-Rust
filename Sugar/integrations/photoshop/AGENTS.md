# Photoshop UXP development

- Source belongs in `src/`; package resources belong in `static/`. `manifest.json` declares the plugin. `build.json` configures repository build tooling.
- `npm run photoshop:build` produces `build/photoshop/`. `npm run photoshop:watch` rebuilds for UXP Developer Tool. Add the source manifest to UDT and configure its distribution directory to the generated output.
- `npm run photoshop:doctor -- --executable <path>` checks an explicitly selected Photoshop executable. Machine paths and local overrides belong in ignored `*.local.json` files or environment variables.
- Pin the minimum Photoshop version and permissions in the manifest when the plugin is scaffolded. Type declarations do not establish runtime availability; verify each used API against the supported host version.
- Native Photoshop and UXP imports belong exclusively in `src/infrastructure/`. Presentation invokes typed application collaborators; entry points remain thin.
- Use the UXP compiler environment for native code and the browser environment for remotely served ComfyUI code. Keep DOM and Node declarations out of portable code.
- Treat WebView messages as untrusted. Validate origin, source, protocol version, session, and payload before dispatch. Use explicit permitted origins and bounded transfers.
- Route document mutations through one infrastructure owner using Photoshop modal execution and coherent undo behavior. Never keep a modal scope open during generation.
- Keep host lifecycle cleanup explicit. Report loading, disconnected, and failed states through owned UI and structured diagnostics.
- Release output excludes source maps and development inspector settings. Package the validated directory using UXP Developer Tool, then verify the installed `.ccx` in Photoshop.
- Run `npm run check:photoshop` for focused validation. Host behavior requires real Photoshop verification of docking, resizing, hide/show, reload, and reconnect. Shared ComfyUI presentation changes also require the repository's browser/renderer matrix.
