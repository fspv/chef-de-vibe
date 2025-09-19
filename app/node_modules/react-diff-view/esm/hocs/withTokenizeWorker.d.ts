import { ComponentType } from 'react';
import { TokenizePayload, TokenizeResult, TokenizeWorkerOptions } from '../hooks';
import { HunkData, Source } from '../utils';
export interface RequiredProps {
    hunks: HunkData[];
    oldSource: Source;
    language: string;
}
interface ToeknizeWorkerHocOptions<T extends TokenizePayload> extends TokenizeWorkerOptions<T> {
    mapPayload?: <P extends RequiredProps>(payload: RequiredProps, props: P) => any;
}
export default function withTokenizeWorkerwithTokenizeWorker<T extends TokenizePayload>(worker: Worker, options?: ToeknizeWorkerHocOptions<T>): <P>(ComponentIn: ComponentType<P & TokenizeResult>) => {
    (props: P & RequiredProps): import("react/jsx-runtime").JSX.Element;
    displayName: string;
};
export {};
