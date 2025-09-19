import { Hunk } from 'gitdiff-parser';
import { ChangeEventArgs } from '../context';
export interface UseChangeSelectOptions {
    multiple?: boolean;
}
export default function useChangeSelect(hunks: Hunk[], { multiple }?: UseChangeSelectOptions): readonly [string[], ({ change }: ChangeEventArgs) => void];
