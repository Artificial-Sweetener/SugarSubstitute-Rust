# Editor integrations

- `contracts/` owns language-neutral wire schemas and portable value declarations. Version protocol changes and keep one authoritative schema when messages are introduced.
- `shared/` owns portable adapter mechanics and conformance support. It may depend on contracts; it may not import an editor, browser, ComfyUI, filesystem, or subprocess implementation.
- `<editor>/` owns that editor's presentation, native API translation, transport, and package. Cross-editor imports are prohibited.
- SugarCubes application code owns capture, generation, and placement sequencing. Editor adapters implement capabilities and primitives through the shared contract.
- Keep document identity, revisions, coordinates, pixel format, color space, cancellation, and unsupported capabilities explicit when adding operations.
- Shared contract changes must select every adapter's subscribed proofs through `TEST_POLICY.toml`. Add a limited in-memory host to the common conformance suite when the first runtime contract is introduced.
- Author runtime behavior in TypeScript, or the editor's required language with equivalent strict checks. Generated JavaScript belongs in an ignored build directory.
- Every new editor must register source ownership, checks, build output, and conformance subscriptions before adding runtime code. Unregistered integrations fail architecture validation.
- Use `npm run check:changed` during local work and `npm run check:staged` for the intended Git index. `npm run check` runs every check and proof in CI.
