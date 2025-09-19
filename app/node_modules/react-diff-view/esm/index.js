export { default as Diff } from './Diff';
export { default as Hunk } from './Hunk';
export { default as Decoration } from './Decoration';
export { computeNewLineNumber, computeOldLineNumber, expandCollapsedBlockBy, expandFromRawCode, findChangeByNewLineNumber, findChangeByOldLineNumber, getChangeKey, getCollapsedLinesCountBetween, getCorrespondingNewLineNumber, getCorrespondingOldLineNumber, insertHunk, parseDiff, textLinesToHunk, isInsert, isDelete, isNormal, } from './utils';
export { markEdits, markWord, pickRanges, tokenize } from './tokenize';
export { minCollapsedLines, withChangeSelect, withSourceExpansion, withTokenizeWorker } from './hocs';
export { useChangeSelect, useMinCollapsedLines, useSourceExpansion, useTokenizeWorker } from './hooks';
//# sourceMappingURL=index.js.map