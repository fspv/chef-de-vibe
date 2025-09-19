import { Change, DeleteChange, File, Hunk, InsertChange, NormalChange } from 'gitdiff-parser';
export declare function isInsert(change: Change): change is InsertChange;
export declare function isDelete(change: Change): change is DeleteChange;
export declare function isNormal(change: Change): change is NormalChange;
export type { File as FileData, Hunk as HunkData, Change as ChangeData };
export interface ParseOptions {
    nearbySequences?: 'zip';
}
export declare function parseDiff(text: string, options?: ParseOptions): File[];
