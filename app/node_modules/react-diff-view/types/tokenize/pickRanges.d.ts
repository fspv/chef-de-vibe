/**
 * @file 在高亮的语法节点上插入代码定义与引用的信息
 * @author zhanglili
 */
import { TokenizeEnhancer } from './interface';
export interface RangeTokenNode {
    type: string;
    lineNumber: number;
    start: number;
    length: number;
}
export default function pickRanges(oldRanges: RangeTokenNode[], newRanges: RangeTokenNode[]): TokenizeEnhancer;
