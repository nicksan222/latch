/** RPC contracts are generated from Rust's protocol::methods declaration. */
import type { MethodMap } from '../../_generated/contracts';
export { METHODS } from '../../_generated/contracts';
export type { MethodMap } from '../../_generated/contracts';

export type Method = keyof MethodMap;
export type ParamsOf<K extends Method> = MethodMap[K]['params'];
export type ResultOf<K extends Method> = MethodMap[K]['result'];
/** Variadic params: methods whose params are `void` take no argument. */
export type ParamsArg<K extends Method> = ParamsOf<K> extends void ? [] : [params: ParamsOf<K>];
