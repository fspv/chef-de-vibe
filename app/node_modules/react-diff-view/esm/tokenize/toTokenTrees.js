import { flatMap, keyBy } from 'lodash';
import { computeOldLineNumber, computeNewLineNumber, isDelete, isInsert, isNormal } from '../utils';
// This function mutates `linesOfCode` argument.
function applyHunk(linesOfCode, { newStart, changes }) {
    // Within each hunk, changes are continous, so we can use a sequential algorithm here.
    //
    // When `linesOfCode` is received here, it has already patched by previous hunk,
    // thus the starting line number has changed due to possible unbanlanced deletions and insertions,
    // we should use `newStart` as the first line number of current reduce.
    const [patchedLines] = changes.reduce(([lines, cursor], change) => {
        if (isDelete(change)) {
            lines.splice(cursor, 1);
            return [lines, cursor];
        }
        if (isInsert(change)) {
            lines.splice(cursor, 0, change.content);
        }
        return [lines, cursor + 1];
    }, [linesOfCode, newStart - 1]);
    return patchedLines;
}
function applyDiff(oldSource, hunks) {
    // `hunks` must be ordered here.
    const patchedLines = hunks.reduce(applyHunk, oldSource.split('\n'));
    return patchedLines.join('\n');
}
function mapChanges(changes, side, toValue) {
    if (!changes.length) {
        return [];
    }
    const computeLineNumber = side === 'old' ? computeOldLineNumber : computeNewLineNumber;
    const changesByLineNumber = keyBy(changes, computeLineNumber);
    const maxLineNumber = computeLineNumber(changes[changes.length - 1]);
    // TODO: why don't we start from the first change's line number?
    return Array.from({ length: maxLineNumber }).map((value, i) => toValue(changesByLineNumber[i + 1]));
}
function groupChanges(hunks) {
    const changes = flatMap(hunks, hunk => hunk.changes);
    return changes.reduce(([oldChanges, newChanges], change) => {
        if (isNormal(change)) {
            oldChanges.push(change);
            newChanges.push(change);
        }
        else if (isDelete(change)) {
            oldChanges.push(change);
        }
        else {
            newChanges.push(change);
        }
        return [oldChanges, newChanges];
    }, [[], []]);
}
function toTextPair(hunks) {
    const [oldChanges, newChanges] = groupChanges(hunks);
    const toText = (change) => (change ? change.content : '');
    const oldText = mapChanges(oldChanges, 'old', toText).join('\n');
    const newText = mapChanges(newChanges, 'new', toText).join('\n');
    return [oldText, newText];
}
function createRoot(children) {
    return { type: 'root', children: children };
}
export default function toTokenTrees(hunks, options) {
    if (options.oldSource) {
        const newSource = applyDiff(options.oldSource, hunks);
        const highlightText = options.highlight
            ? (text) => options.refractor.highlight(text, options.language)
            : (text) => [{ type: 'text', value: text }];
        return [
            createRoot(highlightText(options.oldSource)),
            createRoot(highlightText(newSource)),
        ];
    }
    const [oldText, newText] = toTextPair(hunks);
    const toTree = options.highlight
        ? (text) => createRoot(options.refractor.highlight(text, options.language))
        : (text) => createRoot([{ type: 'text', value: text }]);
    return [toTree(oldText), toTree(newText)];
}
//# sourceMappingURL=toTokenTrees.js.map