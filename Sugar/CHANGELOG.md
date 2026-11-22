## [0.35.0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.34.0...sugarcubes-v0.35.0) (2026-11-22)

### Features

* **canvas:** count the preview frames the canvas accepts in its diagnostics ([2fe0895](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/2fe0895ffeea857e3e164fc4fad3bcca5ee1109b))
* **session:** desktop checkpoint version 2 keeps each workflow's masonry scroll ([8503f51](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/8503f51930749777c9f088c45d8da9a01ddd0e17))
* **sugarcubes:** one owner for edit commands, workflow-owned masonry view and continuous generation ([11e157e](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/11e157e894728bcd92e98bed65be587d35903f21))

### Bug Fixes

* **canvas:** keep the last preview when a run stops and never show an older one ([2a2ff95](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/2a2ff958d0c7eeac4f90ee1c53f8d51da99f91fe))
* **cube-stack:** a bordered floating stack sits one card gap from the cards ([aa57278](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/aa572784d20ac5a1318ccc0fa85d0f3a2f5e161c))
* **output-canvas:** a picked final holds over previews, and the preview tab comes back after a stop ([edcc899](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/edcc89953271706c2d40aae50980c80b4d3cc570))
* **output-canvas:** the web canvas shows a stopped stage's last preview again ([e99b170](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/e99b170040b7a527e2cce3fe768c13cd6e014a85))
* **sugarcubes:** Cancel ends a continuous loop between runs; masonry scroll lands once reachable ([d28148a](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/d28148a9eea179d856216af3c434461559097efe))
* **sugarcubes:** close the tester feedback round ([e5244b5](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/e5244b57864085ad5d2eb12875114493e2966776))
* **sugarcubes:** draw the queue's video badge with Fluent's regular Play icon ([2eba1a4](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/2eba1a4be12dea3317f4f5e2c4681e874890c7de))
* **sugarcubes:** edit menus on ComfyUI cards, restore in checkpoint order, kept masonry scroll ([1e5fb30](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/1e5fb300184714d1cf9899b37d22055b43e4592c))
* **sugarcubes:** open the console on ComfyUI's console key without a competing keybinding ([a253c03](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/a253c03bded73b84b10d6b50f393432e963dd26a))
* **sugarcubes:** quiet Stop, frameless header buttons, Fluent icons and calmer panels ([6ac5a2d](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/6ac5a2d376d656fe38204b50b361acac7f7596f3))
* **sugarcubes:** stack gap in pixels, and the pinned queue always slides out ([3f4536f](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/3f4536feaa06f4d290bc8b3159351429b3d7580b))
* **workflow:** a view switch on an unedited workflow shows it unsaved ([08d1d84](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/08d1d844bc7f47bd7f8c8f2555c7c77c107f51b2))

## [0.34.0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.33.2...sugarcubes-v0.34.0) (2026-11-20)

### Features

* **projects:** open and save Sugar scripts in the host's own file dialog ([4d13213](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/4d13213e855c2f3ba99cfb09a24f4c03ce3990a7))

### Bug Fixes

* **projects:** open and save a Sugar script in any folder, and open a dropped script as its own workflow ([1fe761e](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/1fe761e5bf8f35ad92a17689f4982217675be517))
* **masonry:** give a Cube card's header actions a standing button face in every style, evenly spaced ([c1e24ca](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/c1e24cab02dd97efc06439bd54fc598ba903a92c))
* **projects:** save a Sugar script where the save dialog shows it ([b2b5efc](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/b2b5efcdab4081dcdfee9bcd553a00a1efc1510d))

## [0.33.2](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.33.1...sugarcubes-v0.33.2) (2026-11-20)

### Bug Fixes

* **canvas:** name each output tab after its Cube, even the bar's ends, and keep the node toolbar off the canvas ([fcf7384](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/fcf7384359a44a9451b2cb8796085d74d120b5c5))
* **frame:** offer exporting a workflow once in the application menu ([d2f28d4](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/d2f28d4250bcb8fc2f852223c2d4f235752020fb))
* **masonry:** show nothing of the graph behind the cards, and make a Cube card's header actions real buttons ([f0d08ea](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/f0d08ea947ac27b7bfd07ab6c5f9394e1cf25e15))
* **queue:** close a menu from its own button, and keep ComfyUI's run progress bar off until it is turned on ([f2eb568](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/f2eb5680dda1698b3054b5d9da70158cc0ce9ac5))

## [0.33.1](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.33.0...sugarcubes-v0.33.1) (2026-11-20)

### Bug Fixes

* **projects:** close a just-saved project without being asked to save it ([8e6ff9b](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/8e6ff9b8a313685751176e1c90f19b0f3a6936d8))

## [0.33.0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.32.2...sugarcubes-v0.33.0) (2026-11-20)

### Features

* **projects:** choose a Sugar script in the system file dialog ([624821e](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/624821ea3ed27c49ad3f01ce165e307bc7beac8f))

### Bug Fixes

* **import:** name the nodes and the node pack a Cube is missing instead of a boundary error ([062b96e](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/062b96e68156ced825c908055f1900110797c666))

## [0.32.2](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.32.1...sugarcubes-v0.32.2) (2026-11-20)

### Bug Fixes

* **host:** hear the desktop through one subscription so no link is refused ([53e4e0a](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/53e4e0a9ce2278313ae4bf63f5867f5a46d11491))

## [0.32.1](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.32.0...sugarcubes-v0.32.1) (2026-11-20)

### Bug Fixes

* **recovery:** take back a ComfyUI-stopped notice once it is out of date ([b9d8c77](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/b9d8c77a22c0ccecff603e82c8888a29db08d8bd))

### Performance Improvements

* **graph:** keep zooming the graph smooth over prompt editors ([736fad8](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/736fad88abe01be424836afbcae5237ebefb76e3))
* **masonry:** keep mounting the stack, scrollbars and graph chrome off the cards ([d8d053d](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/d8d053d8e902641a0f4f79d27da2301a3cc4a559))
* **masonry:** keep the stack width and the surface's name off every card ([b2cf547](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/b2cf5478d9f203c6041a5020e7da82e9f3acc39f))
* **masonry:** restyle only what a view switch changes ([875af16](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/875af16f77b9c507be25b8b969835b8b6cbaa078))
* **masonry:** stop restyling the whole workspace during a view switch ([f8ee83a](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/f8ee83a2db1e86a493a817e149b1b30ef8167d7a))
* **scrollbars:** read overflow for a large remount over several frames ([0aac4cf](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/0aac4cf314fbf34a0cb9075223bbb6bff26bc1cf))
* **session:** tell the host where the view stopped, not every place on the way ([5f37941](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/5f37941fde6b6637205ee0ad113101143e9d129a))
* **session:** validate and measure a captured graph without allocating per value ([fcdb835](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/fcdb8356b8a94e82acd70ccabe200b12539578e1))
* **stack:** keep the rail's fade from restyling every card ([8186032](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/8186032492af8a67c6b2735f36b41026198007ef))

## [0.32.0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.31.0...sugarcubes-v0.32.0) (2026-11-19)

### Features

* **updates:** open the update report from the Updates settings page ([18f5f0f](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/18f5f0f9cbdc8b92f9515c99169fdad24ddc9831))

## [0.31.0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.30.0...sugarcubes-v0.31.0) (2026-11-19)

### Features

* **recovery:** offer to restart ComfyUI from the page instead of the title bar ([385857d](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/385857dd7a4ea9a8b4053b6578c29be0f9cbbd33))

### Performance Improvements

* **workspace:** switch between masonry and graph about three times faster ([616ad85](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/616ad85edad099cb0a785edbd9ac7fc2e8957e69))

## [0.30.0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.29.0...sugarcubes-v0.30.0) (2026-11-19)

### Features

* **tabs:** show a file dragged over the tab bar as the tab it will open ([785350c](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/785350c324968be5d2ae268e0229151ef6ff6abd))

### Bug Fixes

* **canvas:** keep the output canvas docked beside a pinned queue ([93b18cf](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/93b18cf3dd56b1ce544fda5897765435eec15ac1))

## [0.29.0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.28.0...sugarcubes-v0.29.0) (2026-11-19)

### Features

* **notices:** show the desktop's messages as notifications, never in the title bar ([c95acb8](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/c95acb859a5e886f1d67122c2c47bf9ce6024d46))

## [0.28.0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.27.1...sugarcubes-v0.28.0) (2026-11-19)

### Features

* **cart:** drag Cubes in the cart as lifted cards, and show them as the stack does ([f8cf846](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/f8cf84667a3e1682755a47ab4a5ff71c3c79df89))
* **queue:** drag a finished job onto the tab bar to open it as a workflow tab ([5ebf065](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/5ebf0655f9fed91ceaaee7cfd582e6403084ff69))
* **queue:** job details, an asset viewer and run numbers; confirmations that appear ([9c04c58](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/9c04c5869401476fc1b4eeac96dfb0895916b4dd))
* **queue:** lay the generation queue out as SugarSubstitute's own, on ComfyUI's jobs and actions ([6f35764](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/6f35764aea3d784bb94c7f078122652fe280a60e))
* **queue:** reorder waiting jobs by dragging, on one drag gesture shared with the Cube stack ([159eb7d](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/159eb7d416a9adb3a646b2e628a63f93479e5b8d))

## [0.27.1](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.27.0...sugarcubes-v0.27.1) (2026-11-19)

### Bug Fixes

* **appearance:** keep node cards solid in the desktop app, in masonry and in the graph ([989847f](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/989847fe004c4784c538578c9c4170fe2953d7f7))
* **appearance:** soften the graph grid in the desktop app ([993db11](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/993db11df029057fac1bbbf74c09a0880341d69b))
* **frame:** bring back Open workflow, Save, Save as and Export workflow in the application menu ([e5d4d5f](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/e5d4d5fbd06f4d53a45b7d0785403d01b4284a99))
* **import:** open a workflow file dropped anywhere in the window ([6b849cb](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/6b849cb19635886f6e441aca92c707fe9a6057ee))

## [0.27.0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.26.1...sugarcubes-v0.27.0) (2026-11-18)

### Features

* **queue:** resize the docked generation queue from its left edge ([ff1dad6](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/ff1dad60bdcca7d537d4ccb715b4d0d1a452c122))
* **queue:** take seeds from a job that is waiting, running or failed ([ab589e6](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/ab589e6fcdf323c6472b7641ae0cc6c0a6329ddc))

## [0.26.1](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.26.0...sugarcubes-v0.26.1) (2026-11-18)

### Bug Fixes

* **canvas:** keep the input canvas's masks when the canvas is closed and opened again ([d2b8d52](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/d2b8d52bbca5115ccf6b33f2a6aa8e3eb8fa56aa))

## [0.26.0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.25.0...sugarcubes-v0.26.0) (2026-11-18)

### Features

* **canvas:** make everything over the canvas one solid surface ([c1d3927](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/c1d3927e66b9cf0ded3879e0b61b364714333c36))

### Bug Fixes

* **canvas:** keep the editor from showing over the image in a narrow window ([acf9cbc](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/acf9cbccdea56c2bcd070e6cb3bfb06be0cdac1f))

## [0.25.0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.24.0...sugarcubes-v0.25.0) (2026-11-18)

### Features

* **canvas:** blur behind every acrylic surface over the native canvas ([e49ecce](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/e49ecce249a9f356d6fafeeead040d9a0b790463))

## [0.24.0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.23.0...sugarcubes-v0.24.0) (2026-11-18)

### Features

* **projects:** keep a project's masks with it through open and save ([6e337ea](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/6e337ea451ded29b441fbe5fa1817b49eb888d78))
* **queue:** take a finished job's seeds into the current workflow from its row ([739eae2](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/739eae244c07db555e3f9c9581cb24565751a799))

## [0.23.0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.22.0...sugarcubes-v0.23.0) (2026-11-18)

### Features

* **projects:** keep a script's overrides, node links and own values through open and save ([7e1dae5](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/7e1dae5f43f728ace143af57fb7cba0e4a3e3a8f))
* **projects:** open a project's tab in the Cube view ([f766514](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/f7665147f255ae3620f7e95f17155f9151187f53))
* **projects:** open and save project scripts from the orb menu, as SugarSubstitute kept them ([802d1fb](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/802d1fb0f675fe49aa9d8fa3e57b97a6e4080f3c))
* **queue:** give Comfy's job queue the generation queue's Fluent look in the frame ([f1c5b5c](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/f1c5b5c643f2d7afcc275c7f435767bf568b429e))
* **queue:** pin the generation queue as a sidebar and count only waiting jobs ([6e57c7a](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/6e57c7a57529aa699944f649e7778396d1ad8aa0))

### Bug Fixes

* **projects:** connect a script's Cubes as the stack they are ([7889667](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/788966761e6d70d0034ea4c720096aa781948516))
* **sugarscript:** read scripts whose bypassed Cubes carry values, hashes and multi-line text ([cd400fa](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/cd400fa7594185817820f2cd7002367cc15016bb))

## [0.22.0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.21.2...sugarcubes-v0.22.0) (2026-11-18)

### Features

* **host:** show a host's notice of unsaved work as a toast that stays until dismissed ([388e856](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/388e8562e2c1c0ee4b881802c69c852dce164680))

## [0.21.2](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.21.1...sugarcubes-v0.21.2) (2026-11-18)

### Bug Fixes

* **session:** save every edit inside a Cube and restore the saved workflow after an unclean exit ([865dac1](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/865dac172779bae52f82bcc2e91104da97adcc26))

## [0.21.1](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.21.0...sugarcubes-v0.21.1) (2026-11-18)

### Bug Fixes

* **prompt:** a long LoRA name no longer widens the trigger-word menu ([1d6b789](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/1d6b789ed33db84ee51d4d9fe738d83c5b3145eb))

## [0.21.0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.20.0...sugarcubes-v0.21.0) (2026-11-18)

### Features

* **prompt:** schedule a LoRA from the prompt menu and insert trigger words ([108ae6d](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/108ae6d624055b48dfa5a05cdcd04c4578e74bb0))

## [0.20.0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.19.5...sugarcubes-v0.20.0) (2026-11-18)

### Features

* **prompt:** complete LoRA names in the model picker's popup ([a521950](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/a5219504cb4d1e1fc09d729283c39ec6cefc3f5d))

## [0.19.5](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.19.4...sugarcubes-v0.19.5) (2026-11-17)

### Bug Fixes

* **relations:** a pasted Cube keeps the original's links and is linked as its own Cube ([4cc50ec](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/4cc50ec772d9843917b5eeb394563d9b65c02df8))

## [0.19.4](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.19.3...sugarcubes-v0.19.4) (2026-11-17)

### Bug Fixes

* **relations:** a linked prompt folds to its title and link selector ([0570bc2](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/0570bc2cd835f8df24bf80e8889e49a8f2d6e846))

## [0.19.3](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.19.2...sugarcubes-v0.19.3) (2026-11-17)

### Bug Fixes

* **relations:** a linked prompt keeps its box and its link selector on every Cube ([6b86353](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/6b863535aa176559fcdf55a2285302d53459383b))

## [0.19.2](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.19.1...sugarcubes-v0.19.2) (2026-11-17)

### Bug Fixes

* **import:** an image or workflow the Python application saved opens in its own workflow ([201d182](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/201d1820c2176dd227c0fda933ab7a3d3111f4b7))
* **models:** a library update no longer makes every model consumer start over ([b1861fa](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/b1861fa3477d4fddc22c2f619c92904c61d70375))
* **prompt:** LoRA banners stay in place while the model library updates ([f371dc1](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/f371dc111bc9346004cc1ddc1e725d978d463f65))
* **surface:** a node pack's strip no longer displaces Substitute's title bar ([844d0eb](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/844d0ebd0ba764d6a57860b11662e3b88c2e5ca8))

## [0.19.1](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.19.0...sugarcubes-v0.19.1) (2026-11-17)

### Bug Fixes

* **import:** the second Cube of a workflow the Python application saved runs ([c07a3b9](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/c07a3b903c3c160c753a10830cb8e3d73824c18a))

## [0.19.0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.18.0...sugarcubes-v0.19.0) (2026-11-17)

### Features

* **import:** load workflows the Python application saved, with their seeds exact ([6553d19](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/6553d1988120ecd005f771fe8c90740ecbb72c45))
* **seed:** seeds beyond the page's exact range are shown, typed and run exactly ([da7a607](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/da7a607adccf6bc4420da6e83a804eb66f22d136))

## [0.18.0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.17.0...sugarcubes-v0.18.0) (2026-11-16)

### Features

* **prompt-editor:** wildcards from the user's sugar/wildcards folder, as in the Python application ([5038e0e](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/5038e0ebe1951fcaef997a4e2474b9509c0ff975))

## [0.17.0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.16.3...sugarcubes-v0.17.0) (2026-11-16)

### Features

* **surface:** select Fluent and Nodes 2.0 the first time a desktop host presents an install ([7b4aaa3](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/7b4aaa323ecc610845f3b3685afc53138ea38cff))

### Bug Fixes

* **editor-actions:** keep presets in the user's sugar folder ([d84b873](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/d84b873135baa4312f9676bd83ab6d1243d14bff))
* **editor-actions:** state why a sizing is unavailable in its submenu, not as a failure on hover ([eda7551](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/eda7551640db9f4bf6a2f77de5a0c74093a3474e))
* **input-canvas:** bring back the masks a generated image was queued with ([d8c8ba0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/d8c8ba09f6ee255b8360da8452b3488acc0bff7f))
* **input-canvas:** keep a reopened or opened workflow's masks and view ([681ebd5](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/681ebd5b0a1f45855ee9fe604be9c8a259798ca4))
* **surface:** close a tab only on its own middle click and reopen closed workflows from the tab bar ([a156002](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/a156002d402e1cfa08fb1efa53617378f3d97fea))
* **surface:** keep workflow undo from also running on a shortcut the canvas took ([afdeb5e](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/afdeb5e3084da3f0fcc20fb242db7b112737f584))

## [0.16.3](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.16.2...sugarcubes-v0.16.3) (2026-11-16)

### Bug Fixes

* **canvas-editing:** keep undo with the canvas and on the keyboard ([14a64e7](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/14a64e791d06d0f20d3bf989c3a586d9077caa37))
* **input-canvas:** refresh mask previews on every canvas change, with no Output special case ([b99b6d3](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/b99b6d3b0c10e7beba9b29913ba0df9f5e6469c6))
* **output-canvas:** never outline the canvas when it takes keyboard focus ([0e27fb6](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/0e27fb63a01a453dc5243f4bcedffe3834dfbbf5))

## [0.16.2](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.16.1...sugarcubes-v0.16.2) (2026-11-16)

### Bug Fixes

* **execution:** keep the workspace saveable after a Cube run ([57d98b9](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/57d98b94bbd54fd8b1db6ad581ad4d2b8a517da4))
* **input-canvas:** keep mask previews while the canvas shows Output ([65eeb71](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/65eeb7157eba64c9560d800add7a3c6f764536ae))
* **workflow:** report a refused workspace save once, as a save failure ([88076a4](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/88076a49b59b231e3fc5a44149c94b6ac6f9abfa))

## [0.16.1](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.16.0...sugarcubes-v0.16.1) (2026-11-16)

### Bug Fixes

* **update:** find SugarCubes releases on GitHub by their own tags ([8c537e5](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/8c537e5a37cd74173a8c665e67712c7d322e9d26))

## [0.16.0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/sugarcubes-v0.15.3...sugarcubes-v0.16.0) (2026-11-15)

### ⚠ BREAKING CHANGES

* **canvas:** native surface discovery and capability descriptors and camera
notifications require schema 2 within canvas envelope 3. Deploy with the paired
native host containing Rust commit 1a3f1b03d1e202367ac8e3c3ebf1480fce3dc694 or a
compatible descendant. Portable session checkpoints use schema 1.
* **canvas:** Native surface discovery and camera notifications require schema 2 within canvas protocol 3. Upgrade SugarCubes and its native host together. Schema 1 discovery is accepted solely for an empty unsupported response. Persisted session inspection schema 1 and independent presentation observation schema 1 are unchanged.
Validation: build/current generated artifacts, all four exact changed/staged static profiles, 1794 Python tests, 4217 Jest tests, and 22 native companion tests passed. All selected targets were attempted; the three documented missing-Firefox1543 suites retain 29 environmental failures. Paired host contract: 1a3f1b03d1e202367ac8e3c3ebf1480fce3dc694.

### Features

* **appearance:** add Fluent node styling and masonry controls ([220012b](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/220012b1387bbe520a3e085744231ce73212685c))
* **appearance:** add shared visual styles and Spectrum 2 ([277b2df](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/277b2df33112e6f46f3860803c1c57b6a7a1bb52))
* **appearance:** add Substitute-style seed controls ([c3d6d9f](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/c3d6d9f952a62427c6783e013347811c98c1f124))
* **appearance:** use the desktop host's accent as the default accent ([30df1ff](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/30df1ff5d837f1f83c2d3c41545de3d5870e8712))
* **canvas:** add bounded overview and pinned wipe comparison ([d70dff6](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/d70dff65181fdf5f1e49bcc0f7afb6e4661f6779))
* **canvas:** add editing tools, options and contextual toolbars for editing canvases ([18e5522](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/18e55220c4ecbec023964154be20a69796656454))
* **canvas:** add local image reveal and format-aware copying ([93f79b1](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/93f79b1c532782c3b462959bc186c75c185f9726))
* **canvas:** add mask editing, sizing, and PhotoPro refinements ([bb0a019](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/bb0a0191ac5cd6012e77ff624fb267fdfa94971e))
* **canvas:** add native editing and workflow surface messages to the canvas protocol ([04af50c](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/04af50c23848bfa0b633e3cc93bc067d10601806))
* **canvas:** blur native pixels under translucent navigation and keep desktop image output native ([c245316](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/c245316b8efe4621c52805cbc440ff06b680ce2d))
* **canvas:** bound image decoding and resource lifetimes ([130a19f](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/130a19fe7759c96941ab3612708a3095eadb80b9))
* **canvas:** compare outputs on native canvas surfaces ([a3cfadb](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/a3cfadb6a8ffd792fd887a277bffc73435e31543))
* **canvas:** compose browser inspection and reusable controls ([30d313d](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/30d313df0d9ce105ad9ad5b03cb1ab739b076545))
* **canvas:** discover native surface capabilities before attachment ([62c4ee7](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/62c4ee72dd2cb25c9c2c23b6649c3a82444b5d41))
* **canvas:** establish reusable view and overlay contracts ([851ae2e](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/851ae2ebc8b2b8282fadb17e37bccb0e8ebb9f0f))
* **canvas:** fall back to the browser canvas when native discovery is refused ([9322caf](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/9322caf1e63d7235ab647e2a7a0de52dfa63b595))
* **canvas:** implement anchored camera and linked inspection ([c8ddfdb](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/c8ddfdbbc400f389e1b123d66a72b73217f2412b))
* **canvas:** integrate scene navigation and verified outputs ([18a2d99](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/18a2d997db91787e9d7bd35480c560f0616dfd7b))
* **canvas:** integrate shared canvas adapters and native source fields ([41971a5](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/41971a5dd287f5d13c24a602836346b0fb01e2d0))
* **canvas:** integrate workflow-aware graph and masonry workspace ([2ff7b0a](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/2ff7b0aa626da735d4bd2955a96ec87f0be03e37))
* **canvas:** navigate canvases from one glass bar with a sliding selection ([8605dbf](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/8605dbf75992543037659a7095b63c298261e6f3))
* **canvas:** negotiate native backdrop material and keep surface grants across discovery refreshes ([44d89bf](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/44d89bf034d159e2147216d51a3e437a874bc055))
* **canvas:** open and drag outputs from the native canvas ([9126aaf](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/9126aaff6905fe53bfa7f3ecc19cd6cc69224976))
* **canvas:** own workflow catalog and view navigation ([177a898](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/177a8987104d61c08ee023a64f676a06b82c5d47))
* **canvas:** present outputs through negotiated native surfaces ([5ccf5f7](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/5ccf5f73c9757e70169526ae8af96dade57cf055))
* **canvas:** publish web occlusions so native hosts route canvas gestures ([2a0c00b](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/2a0c00bb49f7b8cb16e243903b8726b8a98adbd3))
* **canvas:** queue prompt scenes as separate runs and pick scenes in the output canvas ([bb9fde6](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/bb9fde684ffb6b011c0928dc3e072d7a3e811ffe))
* **canvas:** record cameras settled by host-routed gestures ([b53ca32](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/b53ca32a46d067cff257ce9b37231905dc9a4925))
* **canvas:** restore native canvas cameras per view ([5ce0fa0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/5ce0fa03b114e61f4bfcc51c3d0dc1d45aff91da))
* **canvas:** return inpaints as positioned masked layers ([4694e35](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/4694e356495dfc8efa1042229f691649864451d2))
* **canvas:** reveal completed node images in output canvas ([08f912b](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/08f912ba867b99b495f31f9ee32edc9e0e19cece))
* **canvas:** share native navigation and lock previews to fit ([d1fd4ed](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/d1fd4edd70b0d983df703973c10a6608f3d7c6a0))
* **canvas:** show canvas navigation choices as combo boxes ([51b8f1f](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/51b8f1f76e712aa81326b959da084be257fab4e8))
* **canvas:** show overview grids on native canvas surfaces ([6d66d97](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/6d66d97a1bca2d425edd288d33d4071197dec144))
* **canvas:** show the editing chrome over native canvases that advertise editing ([2688925](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/26889256f5685aa99f93cc330fcda0558da798ee))
* **canvas:** slide workspace visibility and slim divider ([cbfdccf](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/cbfdccf983f11a0fb3ca163c003a90da985ad948))
* **canvas:** stream live previews to native canvas surfaces ([740a6ec](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/740a6ec158cda2a6aac74aa72dd761ed18a0ab99))
* **contracts:** define durable desktop session checkpoints ([e7385a4](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/e7385a496bfc2816d066fc53603ab087fdcd24af))
* **cubes:** integrate native stack reordering and themed controls ([1cda352](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/1cda35270ae71c923a8cf87f22ada5680264d437))
* **desktop:** restore workflows, results and canvas views from native sessions ([8cb1f7f](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/8cb1f7f8d44fac84d19dcc41f3168f9efe6c15a7))
* **execution:** capture workflow submission ownership ([48bb9e6](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/48bb9e6e2b5408ff7c198bb421b72cb028e3e448))
* **execution:** expose runtime class resolution to host integrations ([55f0094](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/55f0094c557d11c4134732a90517688371b17d88))
* **execution:** integrate workflow-owned media consumers ([792ef48](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/792ef489a42984c896c1186d94b2d8a8f230135c))
* **execution:** own workflow results and exact job recovery ([f41cac2](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/f41cac25a9af4fc0f4795a7c324cb5013a30c08e))
* **frame:** add opt-in Substitute presentation ([9e71960](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/9e71960420965abb8bb99f21faa80756e6d2bf2d))
* **frame:** integrate native desktop titlebar presentation ([f93c3ae](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/f93c3aed61ab62d7303080726e296d66ba9cd40e))
* **gimp:** add native plugin and reconcile editor behavior ([001e336](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/001e336a84cb2d2f74f405878f3a00adae1bb12d))
* **gimp:** add native visual style with GTK3 icons ([0abcaea](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/0abcaea3e77c5d82cf546f739a4e63cbb6974bd8))
* **gimp:** add Tauri companion and shared canvas integration ([21d63cf](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/21d63cfea4895b38b99cf2e0fd2fa20bdd68f41d))
* **input-canvas:** associate masks with loaded image sources ([ae983cd](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/ae983cde5554bb85d7715b8668805c7f92603534))
* **input-canvas:** overlay masks on full composition previews ([f14c35f](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/f14c35fcd9e2effe08a9924f17d353bf571ac01b))
* **input:** change the resolution of mask-only input canvases ([a3e198a](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/a3e198ad9b6b9ddfac6c8cf11f429434fa9e00f1))
* **input:** colour regions from the theme accent everywhere ([0b63057](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/0b630570be926f9f625f6be3d11518b68f7d01b2))
* **input:** label, import and check regional masks on input canvases ([2f015df](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/2f015df3b1c59e66cf384bc3be3ae1cdcb8da6e0))
* **input:** link regional prompt separators with their mask regions ([5270b8b](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/5270b8bd4530796f4b62517bdb7f085727493543))
* **input:** offer canvas resolution changes from the node context menu ([c0ba0f1](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/c0ba0f17ea1d3ce8fae38c62bd79e3a4d74684d6))
* **input:** provision workflow input surfaces with an Input/Output canvas route ([0cb4caa](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/0cb4caa4211342adcecec6f4803fd7b370a64fc1))
* integrate SugarPrompt and workflow runtime updates ([77d561f](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/77d561f14d19f89a44f14de88c51f6718af374bd))
* **integrations:** let each host declare the protocols and capabilities it offers in one host description, instead of SugarCubes inferring them from the bridge it finds ([3cea326](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/3cea3262ec8009f267dbf0936cd5d3637eb5759b))
* **integrations:** let hosts without the PhotoPro style leave the PhotoPro palette out of their panel presentation ([7bd3c98](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/7bd3c98c8ab0c22debe05ca88532fa5f075521c7))
* **integrations:** step canvas surface discovery down between releases instead of switching it off ([2f3c35b](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/2f3c35b45d83dfe27cc77b4c42fa40fddf86007e))
* **masonry:** add scrollable workflow presentation ([7d523bf](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/7d523bf8d5988ebc869c69069d1d416c2851f32b))
* **memory:** follow the desktop host's memory budget and store its limit in settings ([8bc3650](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/8bc36500b063a422c18d7cc3ac656e6cf3a8e7a8))
* **model-library:** integrate model discovery and version selection ([74f418d](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/74f418d5dda4a02ce21ece79233c9b3b89de5267))
* **model-roots:** read preserved current and legacy selections ([662c9d4](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/662c9d472b8506c6c6254e7c9c888c751f032bee))
* **output:** add configured image persistence and settings migration ([ff23402](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/ff23402d0c09209137fac58f6570c8c2235f98fa))
* **output:** add workflow-local source saving controls ([dbb0157](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/dbb0157fa2e075ac1a06c2221f3bc6f50593118c))
* **output:** integrate managed image saving and transfers ([1ba1160](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/1ba1160a6f69fe1c3cefa2ccee21515397a3de2c))
* **outputs:** resolve exact media artifacts for restored sessions ([cd1903a](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/cd1903a4ccb7f8a0af49d5ded526e2cdd9061b4a))
* **output:** support exact image dragging from the canvas ([45d56e9](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/45d56e940eedddc0d1833efaaf13d43fd676bee7))
* **photoshop:** add dockable SugarCubes panel and PhotoPro integration ([65b91c8](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/65b91c87be3a7b7c62196c7bb41fffa8cb2a8ad2))
* **relations:** add native global overrides and cube linking ([682e99b](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/682e99bf939037d97c210e1a7c69a362536c6929))
* **search:** add masonry finder matching and navigation model ([9cb5126](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/9cb51263428bf0eb36a2498f553664b41c9c173d))
* **search:** integrate Fluent masonry finder with native fields ([6bb58f4](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/6bb58f4b2e6a9ab31cea7ed25d8d2813327dc438))
* **seed:** reuse accepted seeds across workflow controls ([ad3d4ef](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/ad3d4ef0b99a84e50e1e284a78f0976b2dcb5d3c))
* **substitute:** show What's new inside the app from the published changelog ([357d821](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/357d821129d903b62c9290c6fd077b66ef27d6a8))
* **sugarcubes:** show what's new in a modal after SugarCubes or its host updates ([77e2a36](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/77e2a36eb9c2cc3dadbcdff8fda86c3b7fc839df))
* **surface:** animate masonry transitions and refine native controls ([2e0bbd1](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/2e0bbd160e231733e8d3d9c53d064401ccb8b2d7))
* **ui:** add PhotoPro styling and Plugin Mode ([4090615](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/409061537373558ab199f2a41906346ad289e87f))
* **ui:** integrate Substitute workspace and unified settings ([dd4c6e7](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/dd4c6e737ea0f9818379c6d08a78c1def357c49c))
* **ui:** refine Fluent surfaces and dockable override toolbar ([b52605b](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/b52605bad0e98732e13c64c222c80609fb21c315))
* **ui:** refine Plugin Mode and responsive global overrides ([4bc3bc3](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/4bc3bc381fb285c255354dfcfbfbe5e344e1dccf))
* **updates:** let SugarCubes check, stage and install its own update when no host updates it ([ce7ee62](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/ce7ee62800444ae42be4e0022fa301f0b1a887ed))
* **updates:** say plainly why SugarCubes is not updated in place, and that a prepared update installs at the next ComfyUI start ([747e46a](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/747e46a659d784d94ffca31a26bca651d13607d2))
* **updates:** show a host's update status in a SugarCubes Updates entry, with check now and restart to apply ([5d58d9a](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/5d58d9adf470d2911f780ea3857f40f0e2b450db))
* **updates:** show SugarCubes' own update in the Updates entry when no host updates it ([1c48c1b](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/1c48c1b9e6e49157680d3766be7e97859e122140))

### Bug Fixes

* **appearance:** align Spectrum workflow header controls ([9d5eb24](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/9d5eb24627c28e512aa6f27eb146a338ec5fa7ca))
* **appearance:** apply Spectrum card colors and corners ([eef1e02](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/eef1e02ada718b89c417ed18709d34db1c7895d4))
* **appearance:** carry the desktop host theme through Comfy's palette and graph grid ([24dd3f7](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/24dd3f7095f7ceb8dc1881b4a8862bcb75abb57f))
* **appearance:** complete Spectrum cart controls and shared contracts ([6f1d4f5](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/6f1d4f53812063bcb14c03b6a4fc3f583d55cc76))
* **appearance:** credit Spectrum and verify editor navigation ([ade516a](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/ade516a31756a83b02a309ac521ff92b0a9fa517))
* **appearance:** darken focused Fluent fields through one shared field-surface primitive ([6ca2743](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/6ca274303bb51d1ac816b8eb0eee9331908ca0df))
* **appearance:** honor shared accent in Spectrum focus rings ([6e8649a](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/6e8649a0eb1c4f9f45fbc41759b94cfeec211f73))
* **appearance:** integrate qfluentwidgets accent shades for web controls ([fd6b476](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/fd6b4760fcfdc923692a683d516c7829c6adace3))
* **appearance:** match qfluentwidgets control accent variants ([ceb0206](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/ceb02061d9ca6a96479827e32f1270975f610b85))
* **appearance:** move Spectrum workflow indicator to top ([c3893a3](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/c3893a3fa3d91b2668132ee8ce34ecd185115aa5))
* **appearance:** preserve switch size and theme Spectrum fields ([5548044](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/55480447b7d97fdb5910d3f7c9cdca6c8de42da9))
* **appearance:** reconcile late native numeric controls ([409369f](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/409369f5fd3692096adb9a2324a7c37e17f30d07))
* **appearance:** theme Spectrum floating menu surfaces ([f210564](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/f210564a4d2b339210f37046f8820872a40b4083))
* **appearance:** use Photoshop panel and control grays ([9617236](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/96172364dbdb379e578fca34c95d5b5327b77ed0))
* **appearance:** use Spectrum switches for activation controls ([dd3f6f6](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/dd3f6f608aacf6de15129edfdcec718542dde8e9))
* **architecture:** let compiler audits finish under load and fail only on their own exit status ([8e3fac8](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/8e3fac867b33fa35dc4718f1b8f099951201f3d3))
* **build:** preserve active runtime directory during web rebuilds ([5203703](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/5203703297fd922c324007d6f914d1581877995f))
* **build:** retry transient runtime artifact locks ([967deaf](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/967deaf9905c63f35259441cfa88a9aac5d22d3a))
* **canvas:** align source captions and output actions with Substitute ([c5bae6c](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/c5bae6c9803e7af22eeab8abd176384a023e541e))
* **canvas:** arm image dragging after full-resolution transfer is ready ([166177c](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/166177cbf40102dc15071e1844b27f496df5f4f7))
* **canvas:** await native attachment readiness and release ([e0aa125](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/e0aa125bbb5fe79d7d36c2a1246e924041d36ccf))
* **canvas:** blur the native output image behind the canvas navigation bar ([f098d73](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/f098d73539affe69fff1dd3902eb31d5ff1e6262))
* **canvas:** bundle the editing chrome so SugarCubes loads in ComfyUI ([d1548b6](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/d1548b6ba928c582f544fd1731213022f6cad1b1))
* **canvas:** cancel context reads on timeout and disposal ([bd8b81d](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/bd8b81d2d60bdb38bead0c10cb9647883f316c11))
* **canvas:** dock output inspection on the right ([3faeea7](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/3faeea76761ba6f65bfc348d3ad4e0ff6f7ed4e7))
* **canvas:** drag the contextual toolbar by its grip and keep it where it is dropped ([7acc012](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/7acc0120c3ab3a0a9736fad6bc242d3eb4e3e6bc))
* **canvas:** hold canvas requests the native bridge cannot yet queue and deliver them in order ([b375aae](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/b375aae3e7c3a8dc4073a68e9056c4d3cad6054d))
* **canvas:** keep execution overlays above stable workspace ([fc6c56f](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/fc6c56f0ce9975e735dd98f95f61815714a38c95))
* **canvas:** keep graph layers out of the docked output canvas strip ([5158970](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/5158970b831875f4be859f49c0fbf7f122d0c85d))
* **canvas:** keep the editing dock clear of the Input/Output route tabs ([5bbc5f5](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/5bbc5f5c2d971fe65cb275f2e214f41fe8d1d557))
* **canvas:** keep the editing tool rail clear of the canvas navigation bar ([b309bb6](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/b309bb61da4cb2b88196356298b18a10e31fd2af))
* **canvas:** keep the editor width and give window growth to the output canvas ([c1cdb60](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/c1cdb60506662727fc21eeabde52f500a4775181))
* **canvas:** keep the native canvas shown while its panel resizes ([8422d0e](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/8422d0e80362ec058f331a99da9714263a61d0ff))
* **canvas:** keep viewer keys working when a newer camera supersedes a key zoom ([155b748](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/155b7485cece4b24e1b1804bb0337c7ae2174495))
* **canvas:** leave every cursor over the native canvas to the host ([a2c6ec6](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/a2c6ec6e8c64b19033a730998e371c1d5feda39e))
* **canvas:** let the viewer keys move the Input document before any output exists ([bcd2b9b](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/bcd2b9b3b4a1ae56938e8593ce9c638d75f38393))
* **canvas:** match Substitute context menu ordering ([7f8735d](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/7f8735dcb0ac89b50366dee91a0b5768f434bf0e))
* **canvas:** match Substitute output navigation ([49edab7](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/49edab7a94ab3942295af064119413b1b2b67e30))
* **canvas:** measure the docked native canvas clip through its containing blocks ([48e3217](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/48e3217d274cc136569eacdbe6064f9a93b1b470))
* **canvas:** negotiate exact native inspection cameras with schema 2 ([4da92e0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/4da92e0b4820b8a57f051513c537e8d926ad318c))
* **canvas:** offer rotate and flip as soon as Transform is chosen ([41e9d4b](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/41e9d4b335ecb69421d602634ac39789d5e04525))
* **canvas:** offer the Substitute Output menu, with Open in Photoshop and without camera commands ([b2ba5f9](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/b2ba5f9c14607772c1fd8c3ae1c5f48641b53426))
* **canvas:** paint every scene in the native All overview ([3c82246](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/3c82246a8dc683b1dae68451f3f6e38f0a833051))
* **canvas:** prepare source images before switching presentation ([0213334](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/021333473cecf5496826554167e5944849cb9efe))
* **canvas:** present restored outputs to the native canvas under host-contract result identities ([00a328f](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/00a328f968f90ca7b4d39e06722251d372160a55))
* **canvas:** preserve final fidelity and source lifecycles ([8a11ee8](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/8a11ee8302e85ff921d20af9ca699f7601b63e75))
* **canvas:** preserve overview cell sizes before image dimensions load ([6a47d17](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/6a47d17285e93535bbfdbd31832cf9a3346ed56a))
* **canvas:** preserve workspace focus and truthful admission status ([dfb01dd](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/dfb01dd67ecec2ed7c866cd7811f22d17d17a186))
* **canvas:** qualify previews and preserve exact batch navigation ([82a59bc](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/82a59bc42fd9ccce4473169cc1dcd6de32ee9af2))
* **canvas:** recover native canvases whose preparation times out ([47c182c](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/47c182cdeae034c4a468f1e73eee444f998297d6))
* **canvas:** recover timed-out native canvases over outdated attachments ([66e5bb2](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/66e5bb223e82600bd6b1219b08ead2876f6ac8b1))
* **canvas:** resolve editing icons from their own module after bundling ([b143b55](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/b143b5552ef83699fdbe6acc352afef2d530b15a))
* **canvas:** restore anchored segment pickers and lightweight glass ([622183d](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/622183ddf86827c052cd8b71dfa8d4793300c1a6))
* **canvas:** restore workflow updates and refine output chrome ([03cfa02](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/03cfa0285d65a05248b80ca587dcd5e0f57fe11f))
* **canvas:** send the Transform toolbar's Apply and Cancel as session actions ([54727d0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/54727d06a035ccbc87271cd96a06d49850d91ef6))
* **canvas:** show a load error for native finals the host cannot present ([332c4e6](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/332c4e64137e07e137ba68811054d2818be30098))
* **canvas:** show the native canvas through the window's last pixel row and column ([398896d](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/398896dcb3934ff1ab810fae29b9c3fa233e8edc))
* **canvas:** stop timing out source and image requests that wait behind a slow surface preparation ([ae4e201](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/ae4e20114ea1884e408519d223c9e05bef4a9427))
* **canvas:** wait for the native host to answer instead of failing a slow preparation on a deadline ([1acda0b](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/1acda0be98f021b7b5de2a0812959c5c39e3e8d6))
* **desktop:** apply editor material and fade the Cube stack ([626635d](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/626635d004d4839d9530fc3f25f81adfc260e90d))
* **desktop:** load only the active workflow's graph when restoring tabs ([8c25c5c](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/8c25c5c795f94b78b83b5e1b87a1a5d3223fe0ad))
* **desktop:** restore saved workflow tabs on relaunch without unavailable-graph errors ([96fbc78](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/96fbc780a61decc68443545de37463ccaaed725d))
* **execution:** merge upstream without prompt preparation hooks ([cd00b29](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/cd00b29896ab81d61997c443b546bd1d37d9b213))
* **execution:** preserve host observers for direct cube submissions ([c94ea92](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/c94ea92d63e49766f3989f78c190a4363d717d07))
* **execution:** retain proximity bindings through normalization ([ae4d22d](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/ae4d22d881e0bb56f0e3cd25930704b9f880b265))
* **extension:** keep the ComfyUI install location when the pack folder is a link ([d529b02](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/d529b021549a667063b155a2838e2850b01899b4))
* **frame:** draw generation progress in the accent beneath the app orb ([da9e673](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/da9e67382b670d850a1dc1395c23a107ab89051f))
* **frame:** fade the collapsed stack's wash only over the rail's live width, as Substitute did ([881b533](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/881b533e6e28eff78ea05df6d28ced7dda5f6725))
* **frame:** let the SugarSubstitute workspace reach the window's bottom edge ([394d6ab](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/394d6ab1ee270946d42edecf123d098ca72dbf0b))
* **frame:** match Substitute tab strip controls ([4049281](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/404928157ca9544f3588af264bcfb1fb13df0db0))
* **frame:** move the masonry cards with the cube stack, as Substitute did ([e798db8](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/e798db80ce8a2c1af10d202c98a1f86158acf201))
* **frame:** paint the generation history button with Substitute's overlay states ([bf9b649](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/bf9b6498da4b1b806cb292b042166af8a662cb8f))
* **frame:** polish tab dragging and orb resampling ([651b6c7](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/651b6c71fdece5911326f589e2e9a52a57a03c75))
* **frame:** wash the desktop workspace and chrome over Mica like Substitute ([dcfe37c](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/dcfe37cdcc27076c3451752d10d8e3b5cb94411f))
* **history:** preserve canvas and cube state through undo and redo ([24b7510](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/24b75106356672fd9b647b3168d9d6078ca90392))
* **input-canvas:** choose masks on input-canvas cards with Browse Files and region commands ([ca8b4f9](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/ca8b4f9e70ad5d93af8ff67f0a7d35cefde51e85))
* **input-canvas:** give every open workflow its own input-canvas identity ([c99b1f9](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/c99b1f91be4e7d2e90b5edce4f404402b6864b0b))
* **input-canvas:** open a newly chosen image on Input and give the canvas keyboard focus from cards ([92625bc](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/92625bc2ebe22b6bc208a902474a9a9515cebc79))
* **input-canvas:** place mask channel after source choices ([6a272d8](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/6a272d8f4b9fff4257e8b38325508d9dd3212926))
* **input-canvas:** preview a mask without an associated image as its own grayscale coverage ([50933ac](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/50933ac349c44adef5cfc0e18ef7913e04ecbe64))
* **input-canvas:** recheck source bindings across automatic restoration ([7cb51a6](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/7cb51a662b6e90494000639a4116b990155db59a))
* **input-canvas:** return a mask card to its layer when its picked file is rejected ([df4de89](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/df4de89903caf601105743b031c51a0b6c93522d))
* **input-canvas:** skip linked sources during automatic loading ([b603362](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/b603362bec02843c94bf04e63fecd4524e1d6d7e))
* **input:** capture previews only while the canvas holds their workflow ([48e4477](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/48e44777b6822be4695ee5cc59ae0e51a1a1d8dd))
* **input:** follow the Python app's mask endpoints, imports and channels ([53c0a0c](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/53c0a0c23522954a1831a76d72201696addcaa39))
* **input:** give duplicated workflows their own input documents ([61c6689](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/61c66894e8d935fc3bee55e921a436351b28cf12))
* **input:** key canvas source previews by workflow and drop them on switch ([25bf4ce](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/25bf4ce86482fc60c5ce003fda68b160fcb93e05))
* **input:** seed regional masks from the loader's authored mask files ([e7a3f0a](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/e7a3f0a6263542a095a0780fc8122b55b1372f5f))
* **integrations:** fail canvas surface discovery at once when the host answers with a reply the page cannot read, instead of waiting for the deadline ([aead2b9](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/aead2b9f44a804472b7d2264588f5f5264ef4a01))
* **masonry:** paint the translucent Fluent card fill once in the desktop's Comfy style ([2733d09](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/2733d091c14ed87ba983ce90367fb721b7874526))
* **masonry:** remount cards whose workflow reload replaced their nodes ([9bcb66d](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/9bcb66da36ebbdd4e37e340785a3b51db382215d))
* **masonry:** synchronize graph navigation with view transitions ([0a721e7](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/0a721e70c7485652d69408b7b1edb95e519ffc4b))
* **memory:** never shrink page caches below their standalone allowance when joining a host ([9316959](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/9316959f45c3ff71c5fe170b9120b28e145d32a7))
* **menus:** centralize dismissal and enforce shared ownership ([deee226](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/deee226d8f9c2a2a20c031fdbe064b5c25bf81cf))
* **model-library:** preserve Comfy's model folder registry at prestartup ([b1dbe9e](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/b1dbe9e0d2939f155a327a5a907cc2e54ebffbdd))
* **model-library:** report an offline preview refresh as one plain warning ([dce6594](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/dce659442489c77d40a2ee25d269e486b8159df7))
* **models:** report an empty or damaged model file as one named warning and keep inspecting the others ([d772128](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/d7721282e875bd131d14bf24c4b140ed1cd7c169))
* **output-canvas:** fit fresh Run images before inspection ([b449b00](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/b449b0002aec3d4895f2829bb0f28b6219927fa4))
* **output-canvas:** isolate and fit fresh preview assets ([823524f](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/823524f45593a002a7f9aa311b27a7e137b0e5ff))
* **output-canvas:** match Substitute navigation and Run lifecycle ([f62a3b9](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/f62a3b993a98d35f498ee7121ac1bd0f1e52ddfb))
* **output-canvas:** release a canvas the desktop never showed before staging its replacement, so the replacement is not held forever ([d002ea9](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/d002ea902a42d9a900e81edcf0c796d3499f52ba))
* **output:** reject unrelated settings imports ([ffba232](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/ffba2327a534a2be2c5e5ad72471a50c3c47fca8))
* **prompt-editor:** accept suggestions with Tab only ([5a8c12f](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/5a8c12f388817eff64a5006562c2a3e7d0810082))
* **prompt-editor:** construct rich prompts before presentation ([8f0202b](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/8f0202b6617289b673a34e9a87cbdb824cf38021))
* **prompt-editor:** draw the prompt caret so Firefox shows it at every zoom, as Edge does ([59d099d](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/59d099d56e8753691d64d99c9c43ccb4e9e529d6))
* **prompt-editor:** enforce the caret side at soft wraps when Firefox reports touching rows as overlapping ([0e3ee6c](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/0e3ee6cc272908d0b10f82338806bf77a19185b0))
* **prompt-editor:** keep a clicked caret on the clicked wrapped row ([5e790fe](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/5e790fed1d31f3e50a2e8a1bc39630bad68a52b3))
* **prompt-editor:** keep a paste when the clipboard changes during its read ([06b5086](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/06b5086b39836888dc1ac6e3cf18023e15e693ca))
* **prompt-editor:** keep an emphasis at a soft wrap on its row while its arrows step it ([7d83e55](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/7d83e55111488150f227edd7afc82f49ac1544c9))
* **prompt-editor:** keep empty lines steppable when the host page restyles images ([0a8f4b7](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/0a8f4b717feea15ecba9ab21aa034a8b013750b5))
* **prompt-editor:** keep long wrapped prompts from rebuilding on every keystroke ([317d712](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/317d71268a392675b399c26bd624db10f24da8f3))
* **prompt-editor:** keep the clicked caret when a weight edit ends by clicking ([c64040b](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/c64040b7955352fd7e43098a160ba7aff20086b3))
* **prompt-editor:** keep typed text out of regional separator rows ([4de62ba](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/4de62baf07d36e8b239261f297ab055a431c83b5))
* **prompt-editor:** leave an untouched weight as written on Enter ([1b219bf](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/1b219bfe70101932e3fc74f3aafbcdca66c96bbb))
* **prompt-editor:** paint Firefox's caret on the row CodeMirror selects beside widgets ([1e630b4](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/1e630b471452fb474ac7c1ee3ed5da5b3ea26376))
* **prompt-editor:** replace a selection with exactly the typed text ([cb13cb7](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/cb13cb7a2b140c3af0fd8777b04703b308ea8946))
* **prompt-editor:** stop re-measuring settled word groups on every edit ([051885b](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/051885b01cca1db0ca6f5d2b3ed5c01f71f444e4))
* **prompt-editor:** type plain spaces in Firefox ([fddc480](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/fddc48073336367668e3317e55d30ae8366c1f5f))
* **quality:** let a staged change remove a file no area owns, and recheck everything instead of refusing it ([a3c7fd4](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/a3c7fd475354fa87c0d27e2e388bc9ee930c3c50))
* **relations:** place override pin in the card header ([be88460](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/be8846087f35e4451c981e5b2e15cc86ab717763))
* **relations:** prevent duplicate global override cards ([d62a5ff](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/d62a5ff0a140388319b1919561e6e737e0c33e42))
* **relations:** remember override toolbar placement and disclosure ([d9439e9](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/d9439e9f3a623d65fb044eb372a05733568077bb))
* **relations:** retain override docking during execution ([6d58efb](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/6d58efb9d89854f14e64deed040ef3701b3a2166))
* **seed:** open seed options from their dropdown button ([d00258c](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/d00258c457ce589c2e5dbf704dcf5b06a9a2f90c))
* **session:** end a restore that a graph load supersedes as cancelled instead of showing an output canvas error ([d666067](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/d666067f90e50ab1bdd1c30d24b72e26ad33cfd6))
* **session:** keep a restored workflow's saved source identity instead of renaming it to its current path ([4905822](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/49058221834f5bb7c3fbdc5937d3d4ef9e1ffd43))
* **session:** keep the workflow tabs Comfy reopened instead of replacing them from the desktop checkpoint ([2b345eb](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/2b345eb6a96297c09f07491d9fd497d5d29a7c4d))
* **session:** name the rule a refused desktop checkpoint breaks ([b8f5089](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/b8f50899f0e6202ed26988b07fb64adf778f1ff3))
* **session:** retry a workspace capture that a workflow outran instead of showing an output canvas error ([5844f8b](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/5844f8b649abd3910c7246b94f32c2af87cd8472))
* **sugarcubes:** cancelling a page that never became ready restores the previous version ([8eb16a2](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/8eb16a21b3334c517120feae15e543dfdbf096f9))
* **surface:** contain legacy cube controls within graph host ([a5d7556](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/a5d7556353ba856baed4684c9ae4972db9c61dc6))
* **surface:** fade stack wash and preserve translucent materials ([c118e74](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/c118e7466ab992fbcb531fac49d27259faf6c867))
* **surface:** pack masonry and the Cube stack only for Compact masonry and Plugin Mode ([aa1fe9a](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/aa1fe9a47243329b3ed3a8db5329aa1da73ba26e))
* **surface:** release Cube faces when a workflow is cleared ([0421885](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/04218852b4f71c8bfd1ed017f61ceca0f6f019b4))
* **surface:** release workflow-bound sessions and node scans on graph replacement ([47ed722](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/47ed72260d3de36bcf55268492c6f9ff5c64c391))
* **surface:** restore a retained prompt editor after its workflow is configured ([8fc0e06](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/8fc0e062d7b25339638de612ea258ad8bd9893f8))
* **tooling:** stop the mypy gate at an interpreter that lacks the declared dependencies ([6ebb6e7](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/6ebb6e73ae39b21dcd2cc45b4df994aec5696137))
* **ui:** correct Fluent controls and independent override docking ([291a59b](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/291a59b7ec1a639ccc9dc8a6e62ff1ffacc789b5))
* **ui:** keep prompt scrollbars aligned and show model chips in editor titles ([d33653b](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/d33653be3512d6e44f5320b267d0ff6d2685b8a4))
* **ui:** merge verified scrollbar alignment and model chips ([b8215fd](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/b8215fdb4f8486b677b1caa60304fbcbb3d9689d))
* **ui:** refine Substitute frame and workflow presentation ([6c0f284](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/6c0f284eb2570669606908e1862eecf5d3a075cb))
* **ui:** stabilize workflow tabs and coalesce surface refreshes ([4e5372a](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/4e5372adf890a4ae42a9b028f045f24526233c8c))
* **updates:** accept a SugarCubes update only after a full ComfyUI start and its first page load ([e576213](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/e5762135017315b7dc995cd4e2b8b99fea205efd))
* **updates:** check both release sources every time and install flagged Registry releases ([b83196b](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/b83196bd19c0855c774ac16611ed95664bfcd606))
* **updates:** confirm a SugarCubes update when the page loads if ComfyUI's start can't be observed ([a5753a6](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/a5753a692ff58590bacc2f41284f79d729a8445a))
* **updates:** find local edits to SugarCubes offline from a digest record kept beside .tracking ([1400cf7](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/1400cf7eeda1b2c8898a7f4c01be5633359cce91))
* **updates:** keep the Updates entry for SugarCubes pages that speak updates version 1 ([b765373](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/b765373d5329e6c7610e5adcfc7fca57e341cc8c))
* **updates:** look for SugarCubes releases in the repository that tags them ([2e3c4ac](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/2e3c4ac6e51043f8146f0f239ebb1f85a0954b7b))
* **updates:** name the product on each Updates line and say in plain words why SugarCubes was put back ([92f3e22](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/92f3e22c59a739f215074949a08c808ffd68bc4b))
* **updates:** stop a SugarCubes update download that trickles in from outlasting its deadline ([86e964b](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/86e964b1acd8c8e1bedaf5c8b0655043e724fe01))
* **workspace:** restore workflow views and streamline tab switching ([27b6025](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/27b602519ca1055ae9633f2dc8d5a59c41b06be0))

### Performance Improvements

* **memory:** keep workflow-independent caches across switches under one page allowance ([ad2c0a8](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/ad2c0a8f749720dc83d5ca395f7abdd7fdb6e2bc))
* **prompt-editor:** build a suggestion's edit only when it is accepted ([6268b1a](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/6268b1a5928173b9faf76e9634db61222a4b4fd9))
* **prompt-editor:** keep the ghost completion mounted while typing refreshes it ([ed65105](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/ed65105f41ef910ec5464d19b021225cc67d8a2f))
* **prompt-editor:** relabel suggestion rows in place while typing ([59f9e30](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/59f9e306bf68e54194cee632caa5ed42b837481b))
* **prompt-editor:** stop repainting the graph canvas on every prompt keystroke ([0029db5](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/0029db5614246adca32543cca385c7ec552e2484))
* **prompt-editor:** stop whole-document restyles on every keystroke ([6859eeb](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/6859eeb52a7e96e25bf30c1fb6208e15e815a590))
* **workflow:** stop re-downloading and re-classifying unchanged workflows on tab switch ([0897527](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/089752743f505e63a35e7807daddd6135888a511))

## [0.15.3](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.15.2...v0.15.3) (2026-10-11)


### Bug Fixes

* **execution:** prepare source noise capture before cube queue validation ([f5ceede](https://github.com/Artificial-Sweetener/SugarCubes/commit/f5ceede20cadbbe4c1c01606c28206956932d687))

## [0.15.2](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.15.1...v0.15.2) (2026-09-27)


### Bug Fixes

* **outputs:** make video Cube outputs available for playback ([9409dbb](https://github.com/Artificial-Sweetener/SugarCubes/commit/9409dbb8c9ebbfbcb3bcaa3721a765247bbdf150))

## [0.15.1](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.15.0...v0.15.1) (2026-09-26)


### Bug Fixes

* **dependencies:** install node packs without repeated catalog reloads ([498dd4a](https://github.com/Artificial-Sweetener/SugarCubes/commit/498dd4a80508c2356ab6b7b862655d25d18391f1))

# [0.15.0](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.14.10...v0.15.0) (2026-09-23)


### Bug Fixes

* **release:** recognize breaking feature subjects ([1299146](https://github.com/Artificial-Sweetener/SugarCubes/commit/1299146ef2f0d5e2a855982d8bba578d91144976))

## [0.14.10](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.14.9...v0.14.10) (2026-09-22)


### Bug Fixes

* **dependencies:** install required node packs without system Git ([8b38caf](https://github.com/Artificial-Sweetener/SugarCubes/commit/8b38cafb17f52c4bb7d185b026582389caa5a642))
* **dependencies:** keep internal Git worker portable ([f46d27b](https://github.com/Artificial-Sweetener/SugarCubes/commit/f46d27bc994d5e20e0495ef5005c1687e01828f9))

## [0.14.9](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.14.8...v0.14.9) (2026-09-22)


### Bug Fixes

* **dependencies:** match SimpleSyrup Prompt Control release ([658f362](https://github.com/Artificial-Sweetener/SugarCubes/commit/658f3627b6e25bc7e06d1d02887006ca11dade40))

## [0.14.8](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.14.7...v0.14.8) (2026-09-22)


### Bug Fixes

* **dependencies:** install safe aliased source archives ([f3fcb3e](https://github.com/Artificial-Sweetener/SugarCubes/commit/f3fcb3e4a3586a192e830e592e964fe73d7fd5b1))

## [0.14.7](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.14.6...v0.14.7) (2026-09-21)


### Bug Fixes

* **dependencies:** install Prompt Control required by SimpleSyrup ([0f04f3e](https://github.com/Artificial-Sweetener/SugarCubes/commit/0f04f3eb52a22be65fe0c7589a507786d2a96221))

## [0.14.6](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.14.5...v0.14.6) (2026-09-20)


### Bug Fixes

* **dependencies:** expose installed state for launch reconciliation ([dffc47b](https://github.com/Artificial-Sweetener/SugarCubes/commit/dffc47b67678ad653655a28011551c166b4538e9))

## [0.14.5](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.14.4...v0.14.5) (2026-09-20)


### Bug Fixes

* **dependencies:** install every cube-required pack from Registry sources ([2290215](https://github.com/Artificial-Sweetener/SugarCubes/commit/229021558d3b62e91950f75a213a92793ae8db0f))

## [0.14.4](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.14.3...v0.14.4) (2026-09-20)


### Bug Fixes

* **dependencies:** update every cube-required node pack ([e845db2](https://github.com/Artificial-Sweetener/SugarCubes/commit/e845db2902371966f05ae08530570c452d1a8e7b))

## [0.14.3](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.14.2...v0.14.3) (2026-09-20)


### Bug Fixes

* **runtime:** keep Cube workflows usable across dependency changes ([04d9cea](https://github.com/Artificial-Sweetener/SugarCubes/commit/04d9cea9796e1e56a5169635d2217cd3c1b10dff))

## [0.14.2](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.14.1...v0.14.2) (2026-09-19)


### Bug Fixes

* **export:** persist nested subgraph defaults ([28c2acc](https://github.com/Artificial-Sweetener/SugarCubes/commit/28c2acc8859c53b1050f9fef54c795216e158312))
* **surface:** hide consumed cube face widgets ([da03987](https://github.com/Artificial-Sweetener/SugarCubes/commit/da03987a48310c9cb610d43562955390e6088b9b))

## [0.14.1](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.14.0...v0.14.1) (2026-09-19)


### Bug Fixes

* **authoring:** persist canonical Cube documents after save ([15e9ab6](https://github.com/Artificial-Sweetener/SugarCubes/commit/15e9ab64ce9099b09ffb7a1f44b839d8fc7603a0))
* **workflows:** preserve graph identity during Cube updates ([bd5088d](https://github.com/Artificial-Sweetener/SugarCubes/commit/bd5088d1d9002ca787e4c1f3a04fa3e887167f39))

# [0.14.0](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.13.0...v0.14.0) (2026-09-11)


### Features

* **dependencies:** acquire exact first-party extension releases ([e91238b](https://github.com/Artificial-Sweetener/SugarCubes/commit/e91238bcebd330628a319b5d735629a6c16998d3))

# [0.13.0](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.12.0...v0.13.0) (2026-09-11)


### Features

* **workflows:** make canonical cube graphs portable and executable ([66d496f](https://github.com/Artificial-Sweetener/SugarCubes/commit/66d496fe0b0066f0a64d85042ea83eb9ee43ffd2))

# [0.12.0](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.11.0...v0.12.0) (2026-08-28)


### Bug Fixes

* **connections:** retain cube magnetism during resize ([74ad88a](https://github.com/Artificial-Sweetener/SugarCubes/commit/74ad88a07b69ed151dc94fa09fc4a96c52f80ddf))
* **connection:** stabilize magnetic Cube ports ([4928519](https://github.com/Artificial-Sweetener/SugarCubes/commit/4928519a40d87dcad6a9ef93d711346fe5d7a2f6))
* **cube:** make series swaps size aware ([30a6859](https://github.com/Artificial-Sweetener/SugarCubes/commit/30a68596c82d86e6792f90dbb527ad60a6697ef8))
* **export:** enforce native cube boundary integrity ([9bcbc45](https://github.com/Artificial-Sweetener/SugarCubes/commit/9bcbc4545fb0c1495d42f19f5fc414e2995969b7))
* **export:** omit filesystem-backed widget values ([d11dbbb](https://github.com/Artificial-Sweetener/SugarCubes/commit/d11dbbb1aacc95215d8ba666935911d0dd8b597f))
* **geometry:** preserve authored layouts across node renderers ([a605190](https://github.com/Artificial-Sweetener/SugarCubes/commit/a605190d7f845fa297776944cdcf22e667625514))
* **governance:** materialize staged snapshots portably ([5f68787](https://github.com/Artificial-Sweetener/SugarCubes/commit/5f68787734cb00aa66eaa6c9e3df985f001974ab))
* **import:** preserve native cube boundary types ([e296558](https://github.com/Artificial-Sweetener/SugarCubes/commit/e2965588c09b2249246505f06f14b79e46dc9b35))
* **loader:** preserve cube widget identities ([c228aa2](https://github.com/Artificial-Sweetener/SugarCubes/commit/c228aa2b4c593a377d750dc29ad0b1686526f3c0))
* **nodes1:** preserve native resize cursor ([50e7286](https://github.com/Artificial-Sweetener/SugarCubes/commit/50e7286c030c1c05f0a8949e4459800aa919f1ff))
* **surface:** autosize hydrated prompt textareas ([2d50c37](https://github.com/Artificial-Sweetener/SugarCubes/commit/2d50c37b38d65a4aa8b99d0586b53ddef958d07f))
* **surface:** darken cube body to 70 percent ([82d7a79](https://github.com/Artificial-Sweetener/SugarCubes/commit/82d7a79a4a4a7e0a2617fb8641a9d6eec11aec86))
* **surface:** derive Nodes 1 cube colors from theme ([a686efd](https://github.com/Artificial-Sweetener/SugarCubes/commit/a686efdf8731b9d43e0a3e50ac87a3d86943a211))
* **surface:** expose native advanced input controls ([ecc5d0b](https://github.com/Artificial-Sweetener/SugarCubes/commit/ecc5d0b1a73e00d1033e11a190c531c4f5c7bc6f))
* **surface:** make preview collections responsive ([63c4c8d](https://github.com/Artificial-Sweetener/SugarCubes/commit/63c4c8d49f0e3ea6e8f82a6ed5f8135e4b6fba2a))
* **surface:** mount activation controls in native headers ([d736db8](https://github.com/Artificial-Sweetener/SugarCubes/commit/d736db821c0d0bd6259d96dd410a37ce5f1517f7))
* **surface:** preserve native card widget gutter ([b3e824e](https://github.com/Artificial-Sweetener/SugarCubes/commit/b3e824e4cf7b9d470325520fa2c8a458902f1dfa))
* **surface:** recover Cube faces across renderer switches ([8af78dd](https://github.com/Artificial-Sweetener/SugarCubes/commit/8af78dd7ad9065b3107a0616097bd4dd097fd3d6))
* **surface:** restore cube face layout and controls ([bb3b3ea](https://github.com/Artificial-Sweetener/SugarCubes/commit/bb3b3eaf349b49a3bbc5e2309a0ae8dc371dceb9))
* **surface:** restore native Nodes 1 card geometry ([28cf07e](https://github.com/Artificial-Sweetener/SugarCubes/commit/28cf07eea8a407fcfb59210f477f69f75d7c538a))
* **surface:** sync cube cards with node colors ([e299131](https://github.com/Artificial-Sweetener/SugarCubes/commit/e29913113747fb8ecae1aaa42264ae3828c46b42))


### Features

* **backend:** review implementation default changes ([36e8602](https://github.com/Artificial-Sweetener/SugarCubes/commit/36e86028db8deaad1b4248478678da498ef9c4e8))
* **comfy:** complete native cube integration ([3c16b5d](https://github.com/Artificial-Sweetener/SugarCubes/commit/3c16b5d127f4377c8456753b3a04bd1399ba11fd))
* **cube-face:** complete native preview and node interactions ([2fc53bb](https://github.com/Artificial-Sweetener/SugarCubes/commit/2fc53bba31f682fbde7cbea0e30b429e206294a1))
* **cube-face:** refine masonry presentation ([24c57e2](https://github.com/Artificial-Sweetener/SugarCubes/commit/24c57e2a80fcdd8e49f92160b88471e7fa82520d))
* **cube-model:** plan implementation default changes ([d1e2eb2](https://github.com/Artificial-Sweetener/SugarCubes/commit/d1e2eb21752e2a1fb3dac20c21d844f06d7801d7))
* **cube:** normalize output surface names ([f957e90](https://github.com/Artificial-Sweetener/SugarCubes/commit/f957e900e65a8e142fe97393df9f8f2bc3f7f1f9))
* **cubes:** add adjacent cube insertion controls ([1ad14ce](https://github.com/Artificial-Sweetener/SugarCubes/commit/1ad14ce2d1ce742c3129d58e58251c09eaf4642c))
* **cubes:** add native subgraph authoring workflow ([d6a4bf8](https://github.com/Artificial-Sweetener/SugarCubes/commit/d6a4bf82e652a620bf2cfae592e008eaccd15118))
* **cubes:** add native subgraph cube nodes ([9f98966](https://github.com/Artificial-Sweetener/SugarCubes/commit/9f9896601e38e787a6cceae4fe50080793013573))
* **cubes:** improve authoring and renderer compatibility ([6b8a9c2](https://github.com/Artificial-Sweetener/SugarCubes/commit/6b8a9c253251158a587f70c2f0b99f01150eca24))
* **graph:** add magnetic cube port connections ([e6c4c4e](https://github.com/Artificial-Sweetener/SugarCubes/commit/e6c4c4ecd15b6522608f70bf59538a2ce104141b))
* **placement:** keep SugarCubes on root workflows ([6ca5fcb](https://github.com/Artificial-Sweetener/SugarCubes/commit/6ca5fcb9b988941faf792b1094310588af27cc0d))
* **toolbox:** add cube version switching ([cde3f08](https://github.com/Artificial-Sweetener/SugarCubes/commit/cde3f0872c7356c9b8eff2c80eb55d9094c2fdc6))
* **ui:** present native SugarCube identity ([e2ea2a4](https://github.com/Artificial-Sweetener/SugarCubes/commit/e2ea2a4f0a8d46eeaac9b5833c32b79d8f3e8a28))
* **ui:** review defaults during implementation save ([7695c66](https://github.com/Artificial-Sweetener/SugarCubes/commit/7695c665d91d812fe89c93b6325e785b35b7f620))
* **workflows:** add native cube authoring and execution ([a968338](https://github.com/Artificial-Sweetener/SugarCubes/commit/a968338b4a91fa3302deeebb9143c802c8977b06))

# [0.11.0](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.10.0...v0.11.0) (2026-07-21)


### Features

* **api:** publish versioned host integration ([f00ef72](https://github.com/Artificial-Sweetener/SugarCubes/commit/f00ef72534bef41cdea60bc02ffb2fb920ddf317))

# [0.10.0](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.9.3...v0.10.0) (2026-07-14)


### Bug Fixes

* **library:** report personal cube git state ([ddcde49](https://github.com/Artificial-Sweetener/SugarCubes/commit/ddcde4978589367a621c03c62df60aec67da38e7))


### Features

* **cubes:** streamline personal authoring and pack promotion ([69a8224](https://github.com/Artificial-Sweetener/SugarCubes/commit/69a8224e2a2c5c0b30f9dcbe4f873dfa34b973f4))

## [0.9.3](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.9.2...v0.9.3) (2026-07-11)


### Bug Fixes

* **export:** validate persisted widget values ([e34ed20](https://github.com/Artificial-Sweetener/SugarCubes/commit/e34ed20a1773fff79103c42c5622f992170845bf))

## [0.9.2](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.9.1...v0.9.2) (2026-07-08)


### Bug Fixes

* **startup:** avoid eager host imports and stale readiness cache ([b3bc325](https://github.com/Artificial-Sweetener/SugarCubes/commit/b3bc32506b3d6a71ba127a0482ffef7c9f8fd380))

## [0.9.1](https://github.com/Artificial-Sweetener/SugarCubes/compare/v0.9.0...v0.9.1) (2026-06-22)


### Bug Fixes

* **release:** prepare SugarCubes registry publication ([a3733c2](https://github.com/Artificial-Sweetener/SugarCubes/commit/a3733c2833d8a767d87b6c04cd91398b40bb3a15))

# Changelog

## v0.9.0

### Features

- Establish the SugarCubes release baseline.
