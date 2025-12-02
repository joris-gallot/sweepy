# Sweepy

Sweepy is a fast tool to analyze JavaScript/TypeScript projects and find unused exports built with [oxc](https://oxc.rs/) and [napi](https://napi.rs/)

> [!WARNING]
> Sweepy is currently in early development, the API and functionality may change in future releases

## Usage

```bash
npm install @sweepy/core --save-dev
```

```ts
import { sweepy } from '@sweepy/core';

const res = sweepy('path/to/root', ['path-to-entry-1', 'path-to-entry-2']);

console.log(res);
// {
//   unusedExports: [
//     { file: 'src/utils.ts', name: 'unusedFunction' },
//     { file: 'src/constants.ts', name: 'UNUSED_CONSTANT' },
//   ],
// }
```

## References

Inspired by [Knip](https://knip.dev/)
