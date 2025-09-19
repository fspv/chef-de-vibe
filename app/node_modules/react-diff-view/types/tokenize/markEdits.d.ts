import { HunkData } from '../utils';
import { TokenizeEnhancer } from './interface';
export type MarkEditsType = 'block' | 'line';
export interface MarkEditsOptions {
    type?: MarkEditsType;
}
export default function markEdits(hunks: HunkData[], { type }?: MarkEditsOptions): TokenizeEnhancer;
