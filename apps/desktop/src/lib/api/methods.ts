import type { DescMethodUnary, MessageInitShape, MessageShape } from '@bufbuild/protobuf';
import { LatchAgent, type Empty } from '@latch/rpc/v1';

export type Method = DescMethodUnary;
export type ParamsOf<K extends Method> = MessageInitShape<K['input']>;
export type ResultOf<K extends Method> = MessageShape<K['output']>;
export type ParamsArg<K extends Method> =
  MessageShape<K['input']> extends Empty ? [params?: ParamsOf<K>] : [params: ParamsOf<K>];
export const METHODS = LatchAgent.methods.filter((method) => method.methodKind === 'unary');
