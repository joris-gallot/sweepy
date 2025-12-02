export const foo = 'foo'

export function bar() {
  return 'bar'
}

export class Baz {
  greet() {
    return 'Hello from Baz'
  }
}

export const myArrowFunction = (x) => x * x

export async function myAsyncFunction() {
  return 'This is an async function'
}

export function* myGeneratorFunction() {
  yield 1
  yield 2
  yield 3
}

export const myTuple = [1, 'one']

export const myUnionType = 'union'

export const myIntersectionType = { a: 1, b: 'two' }
