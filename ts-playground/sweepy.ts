import { sweepy } from '../index.js'
import path from 'node:path'

const root = path.resolve(import.meta.dirname, 'src')
const entry = path.resolve(root, 'index.ts')

const res = sweepy(root, [entry])

console.log(res)
