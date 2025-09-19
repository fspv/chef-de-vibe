import { HunkData, Source } from '../utils';
export default function useSourceExpansion(hunks: HunkData[], oldSource: Source | null): readonly [HunkData[], (start: number, end: number) => void];
