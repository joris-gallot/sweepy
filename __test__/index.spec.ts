import test from 'ava'
import path from 'node:path'
import { sweepy } from '../index'
import { writeFile, glob, mkdtemp, readFile, mkdir } from 'node:fs/promises'
import os from 'node:os'
import { testCases } from './cases'

async function prepareTsProject({ name, indexContent }: { name: string; indexContent: string }) {
  const root = await mkdtemp(path.join(os.tmpdir(), 'sweepy-'))
  const tsProject = path.resolve(import.meta.dirname, 'fixtures', name)

  const files = await Array.fromAsync(glob('**/*.{js,ts,jsx,tsx,vue}', { cwd: tsProject }))

  await Promise.all(
    files.map(async (file) => {
      const src = path.join(tsProject, file)
      const dest = path.join(root, file)
      const destDir = path.dirname(dest)
      await mkdir(destDir, { recursive: true })
      await writeFile(dest, await readFile(src))
    })
  )

  const indexFile = path.join(root, 'index.ts')
  await writeFile(indexFile, indexContent)

  return { root, indexFile }
}

for (const testCase of testCases) {
  test(testCase.title, async (t) => {
    const { root, indexFile } = await prepareTsProject({
      name: testCase.fixture,
      indexContent: testCase.indexContent,
    })

    const res = sweepy(root, [indexFile])

    t.deepEqual(res, {
      reachableFiles: testCase.expectedReachable,
      unusedExports: testCase.expectedUnused,
    })
  })
}
