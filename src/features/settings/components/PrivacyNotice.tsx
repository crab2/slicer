import { Button } from "../../../components/common/Button";

interface PrivacyNoticeProps {
  open: boolean;
  onConfirm: () => void;
  onCancel: () => void;
  isSubmitting?: boolean;
}

export function PrivacyNotice({
  open,
  onConfirm,
  onCancel,
  isSubmitting = false,
}: PrivacyNoticeProps) {
  if (!open) return null;

  return (
    <div className="privacy-overlay" role="presentation">
      <div
        className="privacy-dialog panel"
        role="dialog"
        aria-modal="true"
        aria-labelledby="privacy-notice-title"
      >
        <h2 id="privacy-notice-title">云端模型隐私提示</h2>
        <p className="muted-copy">
          启用模型分析后，OpenDataLoader 从 PDF 中提取的视觉模块图片、直接导入的图片，
          以及旧版本已生成的兼容页面图，将发送到您配置的模型服务（base URL 或自定义 endpoint）。
          新导入 PDF 的整页和结构化文本不会因此发送。
          请确认您信任该服务的数据处理策略。
        </p>
        <p className="muted-copy">
          API 密钥仅保存在系统凭据存储中，不会写入工作区数据库或导出文件。
        </p>
        <div className="privacy-actions">
          <Button variant="primary" onClick={onConfirm} disabled={isSubmitting}>
            {isSubmitting ? "保存中..." : "我已了解并同意"}
          </Button>
          <Button onClick={onCancel} disabled={isSubmitting}>
            取消
          </Button>
        </div>
      </div>
    </div>
  );
}
