import { ToTokenTreeOptions } from './toTokenTrees';
import { HunkData } from '../utils';
import { TokenizeEnhancer, TokenNode } from './interface';
export { default as pickRanges } from './pickRanges';
export { default as markEdits } from './markEdits';
export { default as markWord } from './markWord';
export type { Pair, TextNode, TokenNode, TokenPath, TokenizeEnhancer } from './interface';
export type { MarkEditsOptions, MarkEditsType } from './markEdits';
export type { RangeTokenNode } from './pickRanges';
export type TokenizeOptions = ToTokenTreeOptions & {
    enhancers?: TokenizeEnhancer[];
};
export interface HunkTokens {
    old: TokenNode[][];
    new: TokenNode[][];
}
export declare const tokenize: (hunks: HunkData[], { enhancers, ...options }?: TokenizeOptions) => HunkTokens;
