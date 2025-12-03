# Sweepy

Sweepy is a fast tool to analyze JavaScript/TypeScript projects and find unused exports built with [oxc](https://oxc.rs/) and [napi](https://napi.rs/)

> [!WARNING]
> Sweepy is experimental and under active development

## Usage

```bash
npm install @sweepy/core --save-dev
```

```ts
import { sweepy } from '@sweepy/core';

const result = sweepy('path/to/project-root', ['path/to/entry-1', 'path/to/entry-2']);

console.log(result);
// {
//   unusedExports: [
//     { file: 'src/utils.ts', name: 'unusedFunction' },
//     { file: 'src/constants.ts', name: 'UNUSED_CONSTANT' },
//   ],
// }
```

## References

Inspired by [Knip](https://knip.dev/)

## License
MIT
