import type { highlight } from 'refractor';
import { HunkData } from '../utils';
import { Pair, TokenNode } from './interface';
interface Refractor {
    highlight: typeof highlight;
}
export interface ToTokenTreeOptionsNoHighlight {
    highlight?: false;
    oldSource?: string;
}
export interface ToTokenTreeOptionsHighlight {
    highlight: true;
    refractor: Refractor;
    oldSource?: string;
    language: string;
}
export type ToTokenTreeOptions = ToTokenTreeOptionsNoHighlight | ToTokenTreeOptionsHighlight;
export default function toTokenTrees(hunks: HunkData[], options: ToTokenTreeOptions): Pair<TokenNode>;
export {};
