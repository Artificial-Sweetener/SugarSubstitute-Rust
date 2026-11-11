# Portable editor contracts

- This directory owns schemas and portable types shared by SugarCubes and editor implementations.
- Keep wire values JSON-compatible and language-neutral. Generate language bindings from one schema when messages are added.
- The compiler environment has no DOM, Node, Photoshop, UXP, or ComfyUI globals.
- Declare capabilities independently of editor identity. Optional features must have explicit unsupported results.
- Keep generation policy in SugarCubes application code and transport mechanics in adapters.
