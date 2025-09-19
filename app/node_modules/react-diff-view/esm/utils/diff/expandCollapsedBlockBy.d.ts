import { HunkData } from '../parse';
export type Source = string | string[];
export declare function expandFromRawCode(hunks: HunkData[], source: Source, start: number, end: number): HunkData[];
export declare function getCollapsedLinesCountBetween(previousHunk: HunkData | null, nextHunk: HunkData): number;
type HunkPredicate = (lines: number, oldStart: number, newStart: number) => boolean;
export declare function expandCollapsedBlockBy(hunks: HunkData[], source: Source, predicate: HunkPredicate): HunkData[];
export {};
