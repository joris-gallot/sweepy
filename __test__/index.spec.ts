import test from 'ava'
import path from 'node:path'
import { sweepy } from '../index'

test('exports named', (t) => {
  const root = path.resolve(import.meta.dirname, 'fixtures', 'exports-named')
  const entry = path.join(root, 'index.ts')
  const res = sweepy(root, [entry])

  t.deepEqual(res, {
    reachableFiles: ['exports-named.ts', 'index.ts'],
    unusedExports: [
      {
        file: 'exports-named.ts',
        name: 'Baz',
      },
      {
        file: 'exports-named.ts',
        name: 'MyAbstractClass',
      },
      {
        file: 'exports-named.ts',
        name: 'MyEnum',
      },
      {
        file: 'exports-named.ts',
        name: 'MyInterface',
      },
      {
        file: 'exports-named.ts',
        name: 'MyNamespace',
      },
      {
        file: 'exports-named.ts',
        name: 'MyType',
      },
      {
        file: 'exports-named.ts',
        name: 'bar',
      },
      {
        file: 'exports-named.ts',
        name: 'myArrowFunction',
      },
      {
        file: 'exports-named.ts',
        name: 'myAsyncFunction',
      },
      {
        file: 'exports-named.ts',
        name: 'myConstEnum',
      },
      {
        file: 'exports-named.ts',
        name: 'myDeclaredFunction',
      },
      {
        file: 'exports-named.ts',
        name: 'myGeneratorFunction',
      },
      {
        file: 'exports-named.ts',
        name: 'myIntersectionType',
      },
      {
        file: 'exports-named.ts',
        name: 'myOverloadedFunction',
      },
      {
        file: 'exports-named.ts',
        name: 'myTuple',
      },
      {
        file: 'exports-named.ts',
        name: 'myUnionType',
      },
    ]
  })
})

test('exports all', (t) => {
  const root = path.resolve(import.meta.dirname, 'fixtures', 'exports-all')
  const entry = path.join(root, 'index.ts')
  const res = sweepy(root, [entry])

  t.deepEqual(res, {
    reachableFiles: ['exports-all.ts', 'exports-named.ts', 'index.ts'],
    unusedExports: [
      {
        file: 'exports-all.ts',
        name: 'extra',
      },
      {
        file: 'exports-named.ts',
        name: 'Baz',
      },
      {
        file: 'exports-named.ts',
        name: 'MyAbstractClass',
      },
      {
        file: 'exports-named.ts',
        name: 'MyEnum',
      },
      {
        file: 'exports-named.ts',
        name: 'MyInterface',
      },
      {
        file: 'exports-named.ts',
        name: 'MyNamespace',
      },
      {
        file: 'exports-named.ts',
        name: 'MyType',
      },
      {
        file: 'exports-named.ts',
        name: 'bar',
      },
      {
        file: 'exports-named.ts',
        name: 'myArrowFunction',
      },
      {
        file: 'exports-named.ts',
        name: 'myAsyncFunction',
      },
      {
        file: 'exports-named.ts',
        name: 'myConstEnum',
      },
      {
        file: 'exports-named.ts',
        name: 'myDeclaredFunction',
      },
      {
        file: 'exports-named.ts',
        name: 'myGeneratorFunction',
      },
      {
        file: 'exports-named.ts',
        name: 'myIntersectionType',
      },
      {
        file: 'exports-named.ts',
        name: 'myOverloadedFunction',
      },
      {
        file: 'exports-named.ts',
        name: 'myTuple',
      },
      {
        file: 'exports-named.ts',
        name: 'myUnionType',
      },
    ]
  })
})
