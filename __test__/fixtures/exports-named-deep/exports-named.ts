export const foo = 'foo'

export function bar() {
  return 'bar'
}

export class Baz {
  greet() {
    return 'Hello from Baz'
  }
}

export interface MyInterface {
  id: number
  name: string
}

export type MyType = {
  value: string
}

export enum MyEnum {
  FIRST,
  SECOND,
  THIRD
}

export namespace MyNamespace {
  export function sayHello() {
    return 'Hello from MyNamespace'
  }
}

export const myArrowFunction = (x: number): number => x * x

export async function myAsyncFunction(): Promise<string> {
  return 'This is an async function'
}

export function* myGeneratorFunction() {
  yield 1
  yield 2
  yield 3
}

export const myTuple: [number, string] = [1, 'one']

export const myUnionType: number | string = 'union'

export const myIntersectionType: { a: number } & { b: string } = { a: 1, b: 'two' }

export function myOverloadedFunction(x: number): number
export function myOverloadedFunction(x: string): string
export function myOverloadedFunction(x: number | string): number | string {
  if (typeof x === 'number') {
    return x * 2
  } else {
    return x + x
  }
}

export abstract class MyAbstractClass {
  abstract getName(): string
}

export declare function myDeclaredFunction(param: string): void

export const myConstEnum = {
  A: 1,
  B: 2,
  C: 3
} as const
