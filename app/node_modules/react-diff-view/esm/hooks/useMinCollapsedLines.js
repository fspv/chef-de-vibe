import { useMemo } from 'react';
import { expandCollapsedBlockBy } from '../utils';
export default function useMinCollapsedLines(minLinesExclusive, hunks, oldSource) {
    const renderingHunks = useMemo(() => {
        if (!oldSource) {
            return hunks;
        }
        const predicate = (lines) => lines < minLinesExclusive;
        return expandCollapsedBlockBy(hunks, oldSource, predicate);
    }, [minLinesExclusive, hunks, oldSource]);
    return renderingHunks;
}
//# sourceMappingURL=useMinCollapsedLines.js.map