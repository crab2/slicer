export type ViewId =
  | "workbench"
  | "mediaImport"
  | "mediaManagement"
  | "analysis"
  | "export"
  | "index"
  | "search"
  | "settings";

export interface NavigationItem {
  id: ViewId;
  label: string;
}

export type ReanalysisSelectedKind =
  | "document"
  | "document_batch"
  | "page"
  | "page_batch";

export interface ReanalysisNavigationContext {
  action: "reanalyze";
  source_tab: ViewId;
  return_to: ViewId;
  selected_kind: ReanalysisSelectedKind;
  selected_ids: string[];
  filter: string;
  query: string;
  scroll_anchor: string | null;
  selection_count: number;
}

export type NavigationContext = ReanalysisNavigationContext;

export const navigationItems: NavigationItem[] = [
  { id: "workbench", label: "工作台" },
  { id: "mediaImport", label: "媒体导入" },
  { id: "mediaManagement", label: "媒体管理" },
  { id: "analysis", label: "模型分析" },
  { id: "export", label: "一键导出" },
  { id: "index", label: "BM25 索引" },
  { id: "search", label: "搜索" },
  { id: "settings", label: "设置" },
];
