import { useState } from "react";
import { StatusBadge } from "../../../components/common/StatusBadge";
import { tauriClient } from "../../../lib/tauriClient";
import type {
  ApiServerRuntimeStatus,
  ApiServerStatusDto,
  AppSettingsDto,
} from "../../../types/app";

interface ApiServerSettingsProps {
  settings: AppSettingsDto;
  onUpdateField: <K extends keyof AppSettingsDto>(
    key: K,
    value: AppSettingsDto[K],
  ) => void;
  runtimeStatus: ApiServerStatusDto | null;
  isLoading: boolean;
}

const RUNTIME_LABEL: Record<ApiServerRuntimeStatus, string> = {
  running: "运行中",
  stopped: "已停止",
  failed: "启动失败",
  disabled: "未启用",
};

const RUNTIME_TONE: Record<
  ApiServerRuntimeStatus,
  "success" | "warning" | "neutral"
> = {
  running: "success",
  stopped: "neutral",
  failed: "warning",
  disabled: "neutral",
};

function truncateToken(token: string): string {
  if (token.length <= 12) return token;
  return `${token.slice(0, 4)}…${token.slice(-4)}`;
}

export function ApiServerSettings({
  settings,
  onUpdateField,
  runtimeStatus,
  isLoading,
}: ApiServerSettingsProps) {
  const [tokenDisplay, setTokenDisplay] = useState<string | null>(null);
  const [isResetting, setIsResetting] = useState(false);

  const status: ApiServerRuntimeStatus = runtimeStatus
    ? runtimeStatus.runtime_status
    : settings.api_enabled
      ? "stopped"
      : "disabled";
  const lastError = runtimeStatus?.last_error ?? null;

  const handleResetToken = async () => {
    if (!confirm("重置 token 后，旧 token 将立即失效。是否继续？")) return;
    setIsResetting(true);
    try {
      const newToken = await tauriClient.resetApiToken();
      setTokenDisplay(newToken);
    } catch {
      setTokenDisplay(null);
    } finally {
      setIsResetting(false);
    }
  };

  return (
    <section className="panel setting-row">
      <div>
        <h2>localhost API</h2>
        <p className="muted-copy">
          默认关闭。启用后仅监听 127.0.0.1，重任务需要 token 保护。
        </p>
        <div className="setting-fields">
          <label>
            <span>启用 API</span>
            <select
              value={settings.api_enabled ? "1" : "0"}
              onChange={(e) =>
                onUpdateField("api_enabled", e.target.value === "1")
              }
              disabled={isLoading}
            >
              <option value="0">关闭</option>
              <option value="1">启用</option>
            </select>
          </label>
          <label>
            <span>监听地址</span>
            <input
              type="text"
              value={settings.api_bind_address}
              onChange={(e) =>
                onUpdateField("api_bind_address", e.target.value)
              }
              disabled={isLoading}
            />
          </label>
          <label>
            <span>端口</span>
            <input
              type="number"
              min={1024}
              max={65535}
              value={settings.api_port}
              onChange={(e) => {
                const v = parseInt(e.target.value, 10);
                if (!isNaN(v)) onUpdateField("api_port", v);
              }}
              disabled={isLoading}
            />
          </label>
        </div>
        <div className="setting-field">
          <button
            type="button"
            onClick={handleResetToken}
            disabled={isResetting}
            className="api-token-reset-button"
          >
            {isResetting ? "重置中…" : "重置访问 token"}
          </button>
          {tokenDisplay && (
            <p className="muted-copy" style={{ marginTop: "0.25rem" }}>
              新 token（仅显示一次）：<code>{truncateToken(tokenDisplay)}</code>
            </p>
          )}
        </div>
        <details className="api-endpoint-summary">
          <summary>端点摘要</summary>
          <table>
            <thead>
              <tr>
                <th>方法</th>
                <th>路径</th>
                <th>认证</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td><code>GET</code></td>
                <td><code>/health</code></td>
                <td>—</td>
              </tr>
              <tr>
                <td><code>GET</code></td>
                <td><code>/search?q=…</code></td>
                <td>—</td>
              </tr>
              <tr>
                <td><code>GET</code></td>
                <td><code>/pages/{"{id}"}</code></td>
                <td>—</td>
              </tr>
              <tr>
                <td><code>GET</code></td>
                <td><code>/documents/{"{id}"}</code></td>
                <td>—</td>
              </tr>
              <tr>
                <td><code>POST</code></td>
                <td><code>/indexes/rebuild</code></td>
                <td>Bearer token</td>
              </tr>
            </tbody>
          </table>
        </details>
        {lastError ? (
          <p className="settings-error">
            {lastError.message}
            {lastError.correlation_id ? (
              <>
                <br />
                <span className="muted-copy">
                  correlation_id: {lastError.correlation_id}
                </span>
              </>
            ) : null}
          </p>
        ) : null}
      </div>
      <div className="setting-actions">
        <StatusBadge tone={RUNTIME_TONE[status]}>
          {RUNTIME_LABEL[status]}
        </StatusBadge>
      </div>
    </section>
  );
}
