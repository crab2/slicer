import { StatusBadge } from "../../components/common/StatusBadge";
import type { WorkspaceStatusDto } from "../../types/app";
import { WorkspaceSettings } from "./components/WorkspaceSettings";

const settingsSections = [
  {
    title: "LibreOffice",
    description: "用于后续 Office 文档转换。当前先保留路径与检测入口。",
    meta: "未检测",
  },
  {
    title: "模型",
    description: "Provider、Endpoint、model name 与密钥状态会保存到安全设置层。",
    meta: "未配置",
  },
  {
    title: "并发",
    description: "默认图片 DPI 144，转换并发 2，分析并发 2。",
    meta: "默认",
  },
  {
    title: "localhost API",
    description: "默认关闭；后续启用时仅监听 127.0.0.1 并使用 token 保护重任务。",
    meta: "关闭",
  },
  {
    title: "隐私提示",
    description: "所有数据默认保存在本地工作区；启用云端模型前会提示页面图片发送范围。",
    meta: "本地优先",
  },
];

interface SettingsPageProps {
  workspaceStatus: WorkspaceStatusDto;
  isWorkspaceLoading: boolean;
  onChooseWorkspace: () => void;
}

export function SettingsPage({
  workspaceStatus,
  isWorkspaceLoading,
  onChooseWorkspace,
}: SettingsPageProps) {
  return (
    <div className="settings-list">
      <WorkspaceSettings
        status={workspaceStatus}
        isLoading={isWorkspaceLoading}
        onChooseWorkspace={onChooseWorkspace}
      />
      {settingsSections.map(({ title, description, meta }) => (
        <section className="panel setting-row" key={title}>
          <div>
            <h2>{title}</h2>
            <p className="muted-copy">{description}</p>
          </div>
          <StatusBadge>{meta}</StatusBadge>
        </section>
      ))}
    </div>
  );
}
