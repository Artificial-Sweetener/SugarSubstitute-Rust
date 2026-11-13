# Portable editor contracts

- This directory owns schemas and portable types shared by SugarCubes and editor implementations.
- Keep wire values JSON-compatible and language-neutral. Generate language bindings from one schema when messages are added.
- The compiler environment has no DOM, Node, Photoshop, UXP, or ComfyUI globals.
- Declare capabilities independently of editor identity. Optional features must have explicit unsupported results.
- Keep generation policy in SugarCubes application code and transport mechanics in adapters.
- Protocol evolution is gated by `tests/editor_integrations/contract/protocol_evolution.test.ts`: `released.json` freezes the schema digest of every released protocol version and the stated behaviour of every capability name, so a changed envelope needs a new protocol version beside the old one, and a changed behaviour needs a new capability name. The same test rejects page code that reads a capability or protocol other than through `declares` or `serves`. `tests/editor_integrations/contract/host_conformance.test.ts` and `recorded_host_profiles.test.ts` prove hosts against the conformance kit and every recorded host release.
