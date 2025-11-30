import test from 'ava'
import path from 'node:path'
import { sweepy } from '../index'

test('simple export', (t) => {
  const root = path.resolve(import.meta.dirname, 'fixtures', 'export-simple')
  const entry = path.join(root, 'index.ts')
  const res = sweepy(root, [entry])

  t.deepEqual(res, {
    reachableFiles: ['index.ts', 'export-simple.ts'],
    unusedExports: []
  })
})
