declare module 'vue-virtual-scroller' {
  import { DefineComponent, VNode } from 'vue'

  export const RecycleScroller: DefineComponent<{
    items: any[];
    itemSize: number;
    keyField: string;
    buffer?: number;
  }, {
    default: (props: { item: any; index: number }) => VNode[]
  }, any>
  export const DynamicScroller: DefineComponent<any, any, any>
  export const DynamicScrollerItem: DefineComponent<any, any, any>
}
