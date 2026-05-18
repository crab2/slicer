export type ViewId = "workbench" | "search" | "settings";

export interface NavigationItem {
  id: ViewId;
  label: string;
}

export const navigationItems: NavigationItem[] = [
  { id: "workbench", label: "工作台" },
  { id: "search", label: "搜索" },
  { id: "settings", label: "设置" },
];
