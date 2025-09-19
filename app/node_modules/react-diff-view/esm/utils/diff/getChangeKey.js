import { isNormal, isInsert } from '../parse';
export function getChangeKey(change) {
    if (!change) {
        throw new Error('change is not provided');
    }
    if (isNormal(change)) {
        return `N${change.oldLineNumber}`;
    }
    const prefix = isInsert(change) ? 'I' : 'D';
    return `${prefix}${change.lineNumber}`;
}
//# sourceMappingURL=getChangeKey.js.map