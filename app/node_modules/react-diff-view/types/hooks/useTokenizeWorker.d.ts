import { HunkData } from '../utils';
import { HunkTokens } from '../tokenize';
export interface TokenizePayload {
    hunks: HunkData[];
    oldSource: string | null;
}
export type ShouldTokenize<P extends TokenizePayload> = (current: P, prev: P | undefined) => boolean;
export interface TokenizeWorkerOptions<P extends TokenizePayload> {
    shouldTokenize?: ShouldTokenize<P>;
}
export interface TokenizeResult {
    tokens: HunkTokens | null;
    tokenizationFailReason: string | null;
}
export default function useTokenizeWorker<P extends TokenizePayload>(worker: Worker, payload: P, options?: TokenizeWorkerOptions<P>): TokenizeResult;
