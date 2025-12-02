import test from 'ava'
import path from 'node:path'
import { sweepy } from '../index'
import { writeFile, glob, mkdtemp, readFile } from 'node:fs/promises'
import os from 'node:os'

async function prepareTsProject({ name, indexContent }:{ name: string, indexContent: string }) {
  const root = await mkdtemp(path.join(os.tmpdir(), 'sweepy-'))
  const tsProject = path.resolve(import.meta.dirname, 'fixtures', name)

  const files = await Array.fromAsync(glob('**/*.ts', { cwd: tsProject }))

  await Promise.all(files.map(async (file) => {
    const src = path.join(tsProject, file)
    const dest = path.join(root, file)
    await writeFile(dest, await readFile(src))
  }))

  const indexFile = path.join(root, 'index.ts')
  await writeFile(indexFile, indexContent)

  return { root, indexFile }
}

test('exports named - no imports', async (t) => {
  const {root, indexFile} = await prepareTsProject({
    name: 'exports-named',
    indexContent: '// no imports'
  })

  const res = sweepy(root, [indexFile])

  t.deepEqual(res, {
    reachableFiles: ['index.ts'],
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
        name: 'foo',
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

test('exports named - some import', async (t) => {
  const {root, indexFile} = await prepareTsProject({
    name: 'exports-named',
    indexContent: 'import { foo } from "./exports-named";'
  })

  const res = sweepy(root, [indexFile])

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

test('exports all - no imports', async (t) => {
  const {root, indexFile} = await prepareTsProject({
    name: 'exports-all',
    indexContent: '// no imports'
  })

  const res = sweepy(root, [indexFile])

  t.deepEqual(res, {
    reachableFiles: ['index.ts'],
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
        name: 'foo',
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

test('exports all - some import', async (t) => {
  const {root, indexFile} = await prepareTsProject({
    name: 'exports-all',
    indexContent: 'import { foo } from "./exports-all";'
  })

  const res = sweepy(root, [indexFile])

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
