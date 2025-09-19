# prism-color-variables

Extract prism colors to css variables

## Usage

Theme variables does not include prism base styles, include it first.

You can optinally include a basic color theme in `themes` folder.

```html
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/prismjs@1.17.1/themes/prism.css">
<link rel="stylesheet" href="node_modules/prism-color-variables/variables.css">
<link rel="stylesheet" href="node_modules/prism-color-variables/themes/duracula.css">
```

Or using js imports:

```javascript
import 'prismjs/themes/prism.css';
import 'prism-color-variables/variables.css';
import 'prism-color-variables/themes/duracula.css';
```

By using less it is possible to inline different themes:

```less
@import (inline) "npm://prismjs/themes/prism.css";
@import (inline) "npm://prism-color-variables/themes/visual-studio.css";

@media (prefers-color-scheme: dark) {
    @import (inline) "npm://prism-color-variables/themes/duracula.css";
}
```
