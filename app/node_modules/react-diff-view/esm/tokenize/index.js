import toTokenTrees from './toTokenTrees';
import normalizeToLines from './normalizeToLines';
import backToTree from './backToTree';
export { default as pickRanges } from './pickRanges';
export { default as markEdits } from './markEdits';
export { default as markWord } from './markWord';
export const tokenize = (hunks, { enhancers = [], ...options } = {}) => {
    const [oldTokenTree, newTokenTree] = toTokenTrees(hunks, options);
    const [oldLinesOfPaths, newLinesOfPaths] = [normalizeToLines(oldTokenTree), normalizeToLines(newTokenTree)];
    const enhance = (pair) => enhancers.reduce((input, enhance) => enhance(input), pair);
    const [oldEnhanced, newEnhanced] = enhance([oldLinesOfPaths, newLinesOfPaths]);
    const [oldTrees, newTrees] = [oldEnhanced.map(backToTree), newEnhanced.map(backToTree)];
    return {
        old: oldTrees.map(root => root.children ?? []),
        new: newTrees.map(root => root.children ?? []),
    };
};
//# sourceMappingURL=index.js.map