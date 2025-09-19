import { jsx as _jsx } from "react/jsx-runtime";
import { describe, test, expect } from 'vitest';
import dedent from 'dedent';
import renderer from 'react-test-renderer';
import Diff from '../index';
import Decoration from '../../Decoration';
import { getChangeKey, parseDiff } from '../../utils';
const sample = dedent `
    diff --git a/src/__test__/index.test.jsx b/src/__test__/index.test.jsx
    index 643c2f0..7883597 100644
    --- a/src/__test__/index.test.jsx
    +++ b/src/__test__/index.test.jsx
    @@ -21,3 +21,3 @@ describe('basic test', () => {
         test('App renders correctly', () => {
    -        expect(renderer.create(<App diffText={'deff'} />).toJSON()).toMatchSnapshot();
    +        expect(renderer.create(<App diffText={'diff'} />).toJSON()).toMatchSnapshot();
         });
`;
const [file] = parseDiff(sample);
function DiffSplit({ children }) {
    return (_jsx(Diff, { diffType: "modify", hunks: file.hunks, viewType: 'split', children: children }));
}
function DiffUnified({ children }) {
    return (_jsx(Diff, { diffType: "modify", hunks: file.hunks, viewType: 'unified', children: children }));
}
describe('Diff', () => {
    const renderRawHunks = (hunks) => _jsx("div", { children: JSON.stringify(hunks) });
    test('renders correctly', () => {
        expect(renderer.create(_jsx(DiffSplit, { children: renderRawHunks })).toJSON()).toMatchSnapshot();
    });
    test('unified Diff', () => {
        expect(renderer.create(_jsx(DiffUnified, { children: renderRawHunks })).toJSON()).toMatchSnapshot();
    });
});
describe('Diff with Decoration', () => {
    const renderDecoration = () => _jsx(Decoration, { children: _jsx("div", { children: "xxx" }) });
    test('renders correctly', () => {
        expect(renderer.create(_jsx(DiffSplit, { children: renderDecoration })).toJSON()).toMatchSnapshot();
    });
    test('unified Diff with Decoration', () => {
        expect(renderer.create(_jsx(DiffUnified, { children: renderDecoration })).toJSON()).toMatchSnapshot();
    });
});
const getWidgets = (hunks) => {
    const changes = hunks.flatMap(v => v.changes);
    const longLines = changes.filter(({ content }) => content.length > 20);
    return longLines.reduce((widgets, change) => {
        const changeKey = getChangeKey(change);
        return {
            ...widgets,
            [changeKey]: _jsx("span", { className: "error", children: "Line too long" }),
        };
    }, {});
};
describe('Diff with Widget', () => {
    test('split widget', () => {
        const result = renderer.create(_jsx(Diff, { hunks: file.hunks, widgets: getWidgets(file.hunks), diffType: "modify", viewType: "split" }));
        expect(result.toJSON()).toMatchSnapshot();
    });
    test('unified widget', () => {
        const result = renderer.create(_jsx(Diff, { hunks: file.hunks, widgets: getWidgets(file.hunks), diffType: "modify", viewType: "unified" }));
        expect(result.toJSON()).toMatchSnapshot();
    });
});
//# sourceMappingURL=Diff.test.js.map