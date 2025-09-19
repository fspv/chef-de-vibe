import { HunkData } from '../parse';
export declare function textLinesToHunk(lines: string[], oldStartLine: number, newStartLine: number): HunkData | null;
export declare function insertHunk(hunks: HunkData[], insertion: HunkData): HunkData[];
