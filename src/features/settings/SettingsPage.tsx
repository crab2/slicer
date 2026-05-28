import { useEffect, useState } from "react";
import { Button } from "../../components/common/Button";
import { ErrorMessage } from "../../components/common/ErrorMessage";
import { StatusBadge } from "../../components/common/StatusBadge";
import { tauriClient } from "../../lib/tauriClient";
import type {
  ApiKeyListDto,
  ApiKeyRecordDto,
  ApiServerStatusDto,
  AppSettingsDto,
  WorkspaceStatusDto,
} from "../../types/app";
import { ApiServerSettings } from "./components/ApiServerSettings";
import { WorkspaceSettings } from "./components/WorkspaceSettings";

const LIBREOFFICE_DOWNLOAD_URL = "https://zh-cn.libreoffice.org/download/libreoffice/";

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
  const [settings, setSettings] = useState<AppSettingsDto | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<{ message: string; correlationId?: string | null } | null>(null);
  const [saved, setSaved] = useState(false);
  const [isFindingLibreOffice, setIsFindingLibreOffice] = useState(false);
  const [libreOfficeMessage, setLibreOfficeMessage] = useState<string | null>(null);
  const [apiRuntimeStatus, setApiRuntimeStatus] =
    useState<ApiServerStatusDto | null>(null);

  // API key state
  const [apiKeyInput, setApiKeyInput] = useState("");
  const [apiKeyLabelInput, setApiKeyLabelInput] = useState("");
  const [apiKeyList, setApiKeyList] = useState<ApiKeyListDto>({ keys: [] });
  const [isSavingKey, setIsSavingKey] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setIsLoading(true);
    Promise.all([tauriClient.getAppSettings(), tauriClient.listApiKeys()])
      .then(([s, keys]) => {
        if (!cancelled) {
          setSettings(s);
          setApiKeyList(keys);
        }
      })
      .catch((e) => {
        if (!cancelled) setError(extractError(e));
      })
      .finally(() => {
        if (!cancelled) setIsLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [workspaceStatus.workspace_path, workspaceStatus.status]);

  useEffect(() => {
    if (workspaceStatus.status !== "ready") {
      setApiRuntimeStatus(null);
      return;
    }
    let cancelled = false;
    const fetchStatus = async () => {
      try {
        const s = await tauriClient.getApiServerStatus();
        if (!cancelled) setApiRuntimeStatus(s);
      } catch {
        // 静默忽略：状态会在下一次轮询自然恢复
      }
    };
    fetchStatus();
    const intervalId = window.setInterval(fetchStatus, 2000);
    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, [workspaceStatus.status, workspaceStatus.workspace_path]);

  function validateBeforeSave(current: AppSettingsDto): string | null {
    if (current.default_image_dpi < 72 || current.default_image_dpi > 300) {
      return "默认图片 DPI 须在 72 到 300 之间。";
    }
    if (current.conversion_concurrency < 1 || current.conversion_concurrency > 8) {
      return "转换并发数须在 1 到 8 之间。";
    }
    if (current.analysis_concurrency < 1 || current.analysis_concurrency > 8) {
      return "分析并发数须在 1 到 8 之间。";
    }
    return null;
  }

  async function handleSave() {
    if (!settings) return;
    const validationError = validateBeforeSave(settings);
    if (validationError) {
      setError({ message: validationError });
      return;
    }
    setIsSaving(true);
    setError(null);
    setSaved(false);
    try {
      await tauriClient.saveAppSettings(settings);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      setError(extractError(e));
    } finally {
      setIsSaving(false);
    }
  }

  async function handleSaveApiKey() {
    if (!apiKeyInput.trim()) return;
    setIsSavingKey(true);
    setError(null);
    try {
      const updatedKeys = await tauriClient.addApiKey(
        settings?.model_provider ?? "siliconflow",
        apiKeyLabelInput.trim(),
        apiKeyInput.trim(),
        true,
      );
      setApiKeyList(updatedKeys);
      setApiKeyInput("");
      setApiKeyLabelInput("");
      const updated = await tauriClient.getAppSettings();
      setSettings(updated);
    } catch (e) {
      setError(extractError(e));
    } finally {
      setIsSavingKey(false);
    }
  }

  async function handleActivateApiKey(record: ApiKeyRecordDto) {
    setIsSavingKey(true);
    setError(null);
    try {
      const updatedKeys = await tauriClient.activateApiKey(record.provider, record.key_id);
      setApiKeyList(updatedKeys);
      const updated = await tauriClient.getAppSettings();
      setSettings(updated);
    } catch (e) {
      setError(extractError(e));
    } finally {
      setIsSavingKey(false);
    }
  }

  async function handleDeleteApiKey(record: ApiKeyRecordDto) {
    setIsSavingKey(true);
    setError(null);
    try {
      const updatedKeys = await tauriClient.deleteApiKeyRecord(record.provider, record.key_id);
      setApiKeyList(updatedKeys);
      const updated = await tauriClient.getAppSettings();
      setSettings(updated);
    } catch (e) {
      setError(extractError(e));
    } finally {
      setIsSavingKey(false);
    }
  }

  async function handleFindLibreOfficePath() {
    if (!settings) return;
    setIsFindingLibreOffice(true);
    setError(null);
    setLibreOfficeMessage(null);
    try {
      const path = await tauriClient.findLibreOfficePath();
      if (path) {
        updateField("libreoffice_path", path);
        setLibreOfficeMessage("已找到 LibreOffice，保存设置后生效。");
      } else {
        setLibreOfficeMessage("没有自动找到 LibreOffice。请先安装，或点击“选择目录”手动配置安装目录。");
      }
    } catch (e) {
      setError(extractError(e));
    } finally {
      setIsFindingLibreOffice(false);
    }
  }

  async function handleChooseLibreOfficePath() {
    setError(null);
    setLibreOfficeMessage(null);
    try {
      const selected = await tauriClient.openLibreOfficeDirectoryDialog();
      if (typeof selected === "string") {
        updateField("libreoffice_path", selected);
        setLibreOfficeMessage("已填入所选目录，保存设置后生效。");
      }
    } catch (e) {
      setError(extractError(e));
    }
  }

  async function handleOpenLibreOfficeDownload() {
    setError(null);
    try {
      await tauriClient.openExternalUrl(LIBREOFFICE_DOWNLOAD_URL);
    } catch (e) {
      setError(extractError(e));
    }
  }

  function updateField<K extends keyof AppSettingsDto>(
    key: K,
    value: AppSettingsDto[K],
  ) {
    setSettings((prev) => (prev ? { ...prev, [key]: value } : prev));
  }

  if (isLoading) {
    return (
      <div className="settings-list">
        <WorkspaceSettings
          status={workspaceStatus}
          isLoading={isWorkspaceLoading}
          onChooseWorkspace={onChooseWorkspace}
        />
        <section className="panel">
          <p className="muted-copy">设置加载中...</p>
        </section>
      </div>
    );
  }

  if (!settings) {
    return (
      <div className="settings-list">
        <WorkspaceSettings
          status={workspaceStatus}
          isLoading={isWorkspaceLoading}
          onChooseWorkspace={onChooseWorkspace}
        />
        {error ? <ErrorMessage title="设置加载失败" message={error.message} correlationId={error.correlationId} /> : null}
      </div>
    );
  }

  const currentProvider = settings.model_provider;
  const currentProviderKeys = apiKeyList.keys.filter(
    (item) => item.provider === currentProvider,
  );
  const currentProviderHasActiveKey = currentProviderKeys.some((item) => item.is_active);

  return (
    <div className="settings-list">
      {error ? (
        <ErrorMessage title="设置操作失败" message={error.message} correlationId={error.correlationId} />
      ) : null}

      <WorkspaceSettings
        status={workspaceStatus}
        isLoading={isWorkspaceLoading}
        onChooseWorkspace={onChooseWorkspace}
      />

      {/* LibreOffice */}
      <section className="panel setting-row">
        <div>
          <h2>LibreOffice</h2>
          <p className="muted-copy">
            LibreOffice 是免费的 Office 套件，本应用会调用它把 DOC、DOCX、PPT、PPTX 转成 PDF 后导入。
          </p>
          <p className="setting-help">
            如果还没有安装，请先打开
            <button
              type="button"
              className="link-button setting-help-link"
              onClick={handleOpenLibreOfficeDownload}
            >
              官方下载页
            </button>
            完成安装；安装后可自动搜索，或手动选择 LibreOffice 的安装目录。
          </p>
          <div className="setting-field">
            <label>
              <span>安装目录或 soffice 路径</span>
              <input
                type="text"
                placeholder="C:/Program Files/LibreOffice/program 或 .../soffice.exe"
                value={settings.libreoffice_path ?? ""}
                onChange={(e) => {
                  setLibreOfficeMessage(null);
                  updateField("libreoffice_path", e.target.value || null);
                }}
              />
            </label>
          </div>
          <div className="setting-inline-actions">
            <Button onClick={handleFindLibreOfficePath} disabled={isFindingLibreOffice}>
              {isFindingLibreOffice ? "搜索中" : "自动搜索"}
            </Button>
            <Button onClick={handleChooseLibreOfficePath}>
              选择目录
            </Button>
          </div>
          {libreOfficeMessage ? (
            <p className="setting-message">{libreOfficeMessage}</p>
          ) : null}
        </div>
        <StatusBadge>
          {settings.libreoffice_path ? "已配置" : "未配置"}
        </StatusBadge>
      </section>

      {/* Model */}
      <section className="panel setting-row">
        <div>
          <h2>模型配置</h2>
          <p className="muted-copy">
            Provider、Endpoint、model name 与密钥。启用云端模型前会提示页面图片发送范围。
          </p>
          <div className="setting-fields">
            <label>
              <span>Provider</span>
              <select
                value={settings.model_provider}
                onChange={(e) => updateField("model_provider", e.target.value)}
              >
                <option value="siliconflow">硅基流动 SiliconFlow</option>
                <option value="mimo">MiMo</option>
                <option value="openai">OpenAI</option>
                <option value="anthropic">Anthropic</option>
              </select>
            </label>
            <label>
              <span>Base URL</span>
              <input
                type="text"
                placeholder="https://api.example.com"
                value={settings.base_url}
                onChange={(e) => updateField("base_url", e.target.value)}
              />
            </label>
            <label>
              <span>自定义 Endpoint</span>
              <input
                type="text"
                placeholder="留空使用默认"
                value={settings.custom_endpoint}
                onChange={(e) =>
                  updateField("custom_endpoint", e.target.value)
                }
              />
            </label>
            <label>
              <span>Model Name</span>
              <input
                type="text"
                placeholder="zai-org/GLM-4.6V"
                value={settings.model_name}
                onChange={(e) => updateField("model_name", e.target.value)}
              />
            </label>
          </div>
        </div>
        <StatusBadge tone={currentProviderHasActiveKey ? "success" : "warning"}>
          {currentProviderHasActiveKey ? "密钥已配置" : "密钥未配置"}
        </StatusBadge>
      </section>

      {/* API Key */}
      <section className="panel setting-row">
        <div>
          <h2>API Key</h2>
          <p className="muted-copy">
            通过系统密钥存储保存，不会出现在日志或配置文件中。
          </p>
          <div className="setting-field api-key-field">
            <input
              type="text"
              placeholder="名称，例如 硅基流动主 Key"
              value={apiKeyLabelInput}
              onChange={(e) => setApiKeyLabelInput(e.target.value)}
            />
            <input
              type="password"
              placeholder="输入 API 密钥"
              value={apiKeyInput}
              onChange={(e) => setApiKeyInput(e.target.value)}
            />
            <Button
              onClick={handleSaveApiKey}
              disabled={isSavingKey || !apiKeyInput.trim()}
            >
              {isSavingKey ? "保存中" : "新增并启用"}
            </Button>
          </div>
          <div className="api-key-list">
            {currentProviderKeys.length === 0 ? (
              <p className="muted-copy api-key-empty">当前 Provider 尚未保存 API Key。</p>
            ) : (
              currentProviderKeys.map((record) => (
                <div className="api-key-item" key={record.key_id}>
                  <div>
                    <strong>{record.label}</strong>
                    <span>{record.is_active ? "已启用" : "备用"}</span>
                  </div>
                  <div className="api-key-actions">
                    {!record.is_active ? (
                      <Button
                        onClick={() => void handleActivateApiKey(record)}
                        disabled={isSavingKey}
                      >
                        启用
                      </Button>
                    ) : null}
                    <Button
                      onClick={() => void handleDeleteApiKey(record)}
                      disabled={isSavingKey}
                    >
                      删除
                    </Button>
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
        <StatusBadge tone={currentProviderHasActiveKey ? "success" : "neutral"}>
          {currentProviderHasActiveKey ? "已配置" : "未配置"}
        </StatusBadge>
      </section>

      {/* Concurrency */}
      <section className="panel setting-row">
        <div>
          <h2>并发与图片</h2>
          <p className="muted-copy">
            默认图片 DPI 144，转换并发 2，分析并发 2。
          </p>
          <div className="setting-fields">
            <label>
              <span>默认图片 DPI</span>
              <input
                type="number"
                min={72}
                max={300}
                value={settings.default_image_dpi}
                onChange={(e) => {
                  const v = parseInt(e.target.value, 10);
                  if (!isNaN(v)) updateField("default_image_dpi", v);
                }}
              />
            </label>
            <label>
              <span>转换并发数</span>
              <input
                type="number"
                min={1}
                max={8}
                value={settings.conversion_concurrency}
                onChange={(e) => {
                  const v = parseInt(e.target.value, 10);
                  if (!isNaN(v)) updateField("conversion_concurrency", v);
                }}
              />
            </label>
            <label>
              <span>分析并发数</span>
              <input
                type="number"
                min={1}
                max={8}
                value={settings.analysis_concurrency}
                onChange={(e) => {
                  const v = parseInt(e.target.value, 10);
                  if (!isNaN(v)) updateField("analysis_concurrency", v);
                }}
              />
            </label>
          </div>
        </div>
        <StatusBadge>默认</StatusBadge>
      </section>

      {/* Localhost API */}
      <ApiServerSettings
        settings={settings}
        onUpdateField={updateField}
        runtimeStatus={apiRuntimeStatus}
        isLoading={isSaving}
      />

      {/* Privacy */}
      <section className="panel setting-row">
        <div>
          <h2>隐私提示</h2>
          <p className="muted-copy">
            所有数据默认保存在本地工作区。启用云端模型前会提示页面图片会发送到用户配置的模型服务。
          </p>
        </div>
        <StatusBadge>本地优先</StatusBadge>
      </section>

      {/* Save button */}
      <div className="settings-actions">
        <Button variant="primary" onClick={handleSave} disabled={isSaving}>
          {isSaving ? "保存中" : saved ? "已保存" : "保存设置"}
        </Button>
      </div>
    </div>
  );
}

function extractError(error: unknown): { message: string; correlationId?: string | null } {
  if (typeof error === "object" && error !== null) {
    const e = error as Record<string, unknown>;
    const msg = typeof e.message === "string" ? e.message : null;
    const cid = typeof e.correlation_id === "string" ? e.correlation_id : null;
    if (msg) return { message: msg, correlationId: cid };
  }
  if (error instanceof Error) return { message: error.message };
  if (typeof error === "string") return { message: error };
  return { message: "设置操作失败，请稍后重试。" };
}
