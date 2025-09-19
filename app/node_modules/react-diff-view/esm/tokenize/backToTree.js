import { last, isEqual, isEqualWith } from 'lodash';
function areNodesMeregable(x, y) {
    if (x.type !== y.type) {
        return false;
    }
    if (x.type === 'text') {
        return true;
    }
    if (!x.children || !y.children) {
        return false;
    }
    return isEqualWith(x, y, (x, y, name) => (name === 'chlidren' || isEqual(x, y)));
}
function mergeNode(x, y) {
    if ('value' in x && 'value' in y) {
        return {
            ...x,
            value: `${x.value}${y.value}`,
        };
    }
    return x;
}
function attachNode(parent, node) {
    if (!parent.children) {
        throw new Error('parent node missing children property');
    }
    const previousSibling = last(parent.children);
    if (previousSibling && areNodesMeregable(previousSibling, node)) {
        /* eslint-disable no-param-reassign */
        parent.children[parent.children.length - 1] = mergeNode(previousSibling, node);
        /* eslint-enable no-param-reassign */
    }
    else {
        parent.children.push(node);
    }
    const leaf = parent.children[parent.children.length - 1];
    return leaf;
}
export default function backToTree(pathList) {
    const root = { type: 'root', children: [] };
    for (const path of pathList) {
        path.reduce((parent, node, i) => {
            const nodeToUse = i === path.length - 1 ? { ...node } : { ...node, children: [] };
            return attachNode(parent, nodeToUse);
        }, root);
    }
    return root;
}
//# sourceMappingURL=backToTree.js.map