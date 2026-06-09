export type ViewId =
  | "workbench"
  | "imageImport"
  | "analysis"
  | "export"
  | "index"
  | "search"
  | "settings";

export interface NavigationItem {
  id: ViewId;
  label: string;
}

export const navigationItems: NavigationItem[] = [
  { id: "workbench", label: "工作台" },
  { id: "imageImport", label: "图片导入" },
  { id: "analysis", label: "模型分析" },
  { id: "export", label: "一键导出" },
  { id: "index", label: "BM25 索引" },
  { id: "search", label: "搜索" },
  { id: "settings", label: "设置" },
];
