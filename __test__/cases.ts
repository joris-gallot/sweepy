import path from "node:path";

export interface TestCase {
  title: string;
  fixture: string;
  indexContent: string;
  expectedReachable: string[];
  expectedUnused: Array<{ file: string; name: string }>;
}

export const testCases: TestCase[] = [
  // ===== Basic Named Exports =====
  {
    title: 'basic named exports - no imports',
    fixture: 'basic-named',
    indexContent: '// no imports',
    expectedReachable: ['index.ts'],
    expectedUnused: [
      { file: 'utils.ts', name: 'MyClass' },
      { file: 'utils.ts', name: 'MyEnum' },
      { file: 'utils.ts', name: 'MyInterface' },
      { file: 'utils.ts', name: 'MyType' },
      { file: 'utils.ts', name: 'bar' },
      { file: 'utils.ts', name: 'baz' },
      { file: 'utils.ts', name: 'foo' },
      { file: 'utils.ts', name: 'myFunction' },
    ],
  },
  {
    title: 'basic named exports - some imports',
    fixture: 'basic-named',
    indexContent: 'import { foo, bar } from "./utils";',
    expectedReachable: ['index.ts', 'utils.ts'],
    expectedUnused: [
      { file: 'utils.ts', name: 'MyClass' },
      { file: 'utils.ts', name: 'MyEnum' },
      { file: 'utils.ts', name: 'MyInterface' },
      { file: 'utils.ts', name: 'MyType' },
      { file: 'utils.ts', name: 'baz' },
      { file: 'utils.ts', name: 'myFunction' },
    ],
  },
  {
    title: 'basic named exports - all imports',
    fixture: 'basic-named',
    indexContent:
      'import { foo, bar, baz, myFunction, MyClass, MyInterface, MyType, MyEnum } from "./utils";',
    expectedReachable: ['index.ts', 'utils.ts'],
    expectedUnused: [],
  },
  {
    title: 'basic named exports - type-only imports',
    fixture: 'basic-named',
    indexContent:
      'import type { MyInterface, MyType } from "./utils"; import { foo } from "./utils";',
    expectedReachable: ['index.ts', 'utils.ts'],
    expectedUnused: [
      { file: 'utils.ts', name: 'MyClass' },
      { file: 'utils.ts', name: 'MyEnum' },
      { file: 'utils.ts', name: 'bar' },
      { file: 'utils.ts', name: 'baz' },
      { file: 'utils.ts', name: 'myFunction' },
    ],
  },

  // ===== Default Exports =====
  {
    title: 'default export - no imports',
    fixture: 'basic-default',
    indexContent: '// no imports',
    expectedReachable: ['index.ts'],
    expectedUnused: [
      { file: 'utils.ts', name: 'default' },
      { file: 'utils.ts', name: 'namedExport' },
    ],
  },
  {
    title: 'default export - default import only',
    fixture: 'basic-default',
    indexContent: 'import defaultFn from "./utils";',
    expectedReachable: ['index.ts', 'utils.ts'],
    expectedUnused: [{ file: 'utils.ts', name: 'namedExport' }],
  },
  {
    title: 'default export - named import only',
    fixture: 'basic-default',
    indexContent: 'import { namedExport } from "./utils";',
    expectedReachable: ['index.ts', 'utils.ts'],
    expectedUnused: [{ file: 'utils.ts', name: 'default' }],
  },
  {
    title: 'default export - mixed imports',
    fixture: 'basic-default',
    indexContent: 'import defaultFn, { namedExport } from "./utils";',
    expectedReachable: ['index.ts', 'utils.ts'],
    expectedUnused: [],
  },

  // ===== Namespace Imports =====
  {
    title: 'namespace import - marks all exports as used',
    fixture: 'namespace-import',
    indexContent: 'import * as utils from "./utils";',
    expectedReachable: ['index.ts', 'utils.ts'],
    expectedUnused: [],
  },

  // ===== Re-export All =====
  {
    title: 'reexport all - no imports',
    fixture: 'reexport-all',
    indexContent: '// no imports',
    expectedReachable: ['index.ts'],
    expectedUnused: [
      { file: 'barrel.ts', name: 'extra' },
      { file: 'utils.ts', name: 'bar' },
      { file: 'utils.ts', name: 'baz' },
      { file: 'utils.ts', name: 'foo' },
    ],
  },
  {
    title: 'reexport all - import from barrel',
    fixture: 'reexport-all',
    indexContent: 'import { foo, extra } from "./barrel";',
    expectedReachable: ['barrel.ts', 'index.ts', 'utils.ts'],
    expectedUnused: [
      { file: 'utils.ts', name: 'bar' },
      { file: 'utils.ts', name: 'baz' },
    ],
  },
  {
    title: 'reexport all - namespace import from barrel',
    fixture: 'reexport-all',
    indexContent: 'import * as barrel from "./barrel";',
    expectedReachable: ['barrel.ts', 'index.ts', 'utils.ts'],
    expectedUnused: [],
  },

  // ===== Re-export Named =====
  {
    title: 'reexport named - import used reexport',
    fixture: 'reexport-named',
    indexContent: 'import { foo } from "./barrel";',
    expectedReachable: ['barrel.ts', 'index.ts', 'utils.ts'],
    expectedUnused: [
      { file: 'barrel.ts', name: 'bar' },
      { file: 'barrel.ts', name: 'extra' },
      { file: 'utils.ts', name: 'bar' },
      { file: 'utils.ts', name: 'baz' },
      { file: 'utils.ts', name: 'qux' },
    ],
  },
  {
    title: 'reexport named - import non-reexported',
    fixture: 'reexport-named',
    indexContent: 'import { extra } from "./barrel";',
    expectedReachable: ['barrel.ts', 'index.ts', 'utils.ts'],
    expectedUnused: [
      { file: 'barrel.ts', name: 'bar' },
      { file: 'barrel.ts', name: 'foo' },
      { file: 'utils.ts', name: 'bar' },
      { file: 'utils.ts', name: 'baz' },
      { file: 'utils.ts', name: 'foo' },
      { file: 'utils.ts', name: 'qux' },
    ],
  },

  // ===== Deep Paths =====
  {
    title: 'deep paths - relative import',
    fixture: 'deep-paths',
    indexContent: 'import { foo } from "./src/utils/helpers";',
    expectedReachable: ['index.ts', path.join('src', 'utils', 'helpers.ts')],
    expectedUnused: [
      { file: path.join('src','utils','helpers.ts'), name: 'bar' },
      { file: path.join('src','utils','helpers.ts'), name: 'baz' },
    ],
  },

  // ===== Side Effects =====
  {
    title: 'side effects - import for side effects only',
    fixture: 'side-effects',
    indexContent: 'import "./setup";',
    expectedReachable: ['index.ts', 'setup.ts'],
    expectedUnused: [
      { file: 'setup.ts', name: 'config' },
      { file: 'setup.ts', name: 'initialize' },
    ],
  },
  {
    title: 'side effects - import with named exports',
    fixture: 'side-effects',
    indexContent: 'import { config } from "./setup";',
    expectedReachable: ['index.ts', 'setup.ts'],
    expectedUnused: [{ file: 'setup.ts', name: 'initialize' }],
  },

  // ===== Mixed Extensions =====
  {
    title: 'js import - import js from ts',
    fixture: 'js-import',
    indexContent: 'import { foo } from "./utils";',
    expectedReachable: ['index.ts', 'utils.js'],
    expectedUnused: [
      { file: 'utils.js', name: 'bar' },
      { file: 'utils.js', name: 'baz' },
      { file: 'utils.js', name: 'jsFunction' },
    ],
  },
  {
    title: 'jsx import - import jsx component',
    fixture: 'jsx-import',
    indexContent: 'import { Component } from "./component";',
    expectedReachable: ['component.jsx', 'index.ts'],
    expectedUnused: [
      { file: 'component.jsx', name: 'AnotherComponent' },
      { file: 'component.jsx', name: 'unusedComponent' },
    ],
  },

  // ===== Vue Files =====
  {
    title: 'vue basic - no imports',
    fixture: 'vue-basic',
    indexContent: '// no imports',
    expectedReachable: ['index.ts'],
    expectedUnused: [
      { file: 'Component.vue', name: 'unusedHelper' },
      { file: 'Component.vue', name: 'useCounter' },
      { file: 'Component.vue', name: 'useStore' },
    ],
  },
  {
    title: 'vue basic - some imports',
    fixture: 'vue-basic',
    indexContent: 'import { useCounter } from "./Component.vue";',
    expectedReachable: ['Component.vue', 'index.ts'],
    expectedUnused: [
      { file: 'Component.vue', name: 'unusedHelper' },
      { file: 'Component.vue', name: 'useStore' },
    ],
  },
  {
    title: 'vue basic - all imports',
    fixture: 'vue-basic',
    indexContent: 'import { useCounter, useStore, unusedHelper } from "./Component.vue";',
    expectedReachable: ['Component.vue', 'index.ts'],
    expectedUnused: [],
  },
  {
    title: 'vue mixed - vue imports from ts',
    fixture: 'vue-mixed',
    indexContent: 'import { useComponent } from "./Component.vue";',
    expectedReachable: ['Component.vue', 'index.ts', 'utils.ts'],
    expectedUnused: [
      { file: 'Component.vue', name: 'ComponentName' },
      { file: 'utils.ts', name: 'ApiResponse' },
      { file: 'utils.ts', name: 'User' },
      { file: 'utils.ts', name: 'formatDate' },
      { file: 'utils.ts', name: 'unusedUtilFunction' },
    ],
  },
  {
    title: 'vue mixed - ts imports types from vue',
    fixture: 'vue-mixed',
    indexContent: 'import { ComponentName } from "./Component.vue";',
    expectedReachable: ['Component.vue', 'index.ts', 'utils.ts'],
    expectedUnused: [
      { file: 'Component.vue', name: 'useComponent' },
      { file: 'utils.ts', name: 'ApiResponse' },
      { file: 'utils.ts', name: 'User' },
      { file: 'utils.ts', name: 'formatDate' },
      { file: 'utils.ts', name: 'unusedUtilFunction' },
    ],
  },
  {
    title: 'vue chain - vue imports vue imports ts',
    fixture: 'vue-chain',
    indexContent: 'import { App } from "./App.vue";',
    expectedReachable: ['App.vue', 'Child.vue', 'api.ts', 'index.ts'],
    expectedUnused: [
      { file: 'App.vue', name: 'useApp' },
      { file: 'Child.vue', name: 'unusedChildExport' },
      { file: 'Child.vue', name: 'useChild' },
      { file: 'api.ts', name: 'ApiConfig' },
      { file: 'api.ts', name: 'config' },
      { file: 'api.ts', name: 'unusedApiFunction' },
    ],
  },
  {
    title: 'vue chain - import without extension',
    fixture: 'vue-chain',
    indexContent: 'import { ChildComponent } from "./Child";',
    expectedReachable: ['Child.vue', 'api.ts', 'index.ts'],
    expectedUnused: [
      { file: 'App.vue', name: 'App' },
      { file: 'App.vue', name: 'useApp' },
      { file: 'Child.vue', name: 'unusedChildExport' },
      { file: 'Child.vue', name: 'useChild' },
      { file: 'api.ts', name: 'ApiConfig' },
      { file: 'api.ts', name: 'config' },
      { file: 'api.ts', name: 'unusedApiFunction' },
    ],
  },
];
