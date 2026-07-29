import { useEffect, useState } from "react";
import { Button } from "../../components/common/Button";
import { ErrorMessage } from "../../components/common/ErrorMessage";
import { StatusBadge } from "../../components/common/StatusBadge";
import { tauriClient } from "../../lib/tauriClient";
import type {
  ApiServerStatusDto,
  AppSettingsDto,
  ModelInfoDto,
  ModelProfileDto,
  ModelProfileListDto,
  ModelProfileUpsertRequestDto,
  WorkspaceStatusDto,
} from "../../types/app";
import { ApiServerSettings } from "./components/ApiServerSettings";
import { WorkspaceSettings } from "./components/WorkspaceSettings";

const LIBREOFFICE_DOWNLOAD_URL = "https://zh-cn.libreoffice.org/download/libreoffice/";

type ModelProfileFormState = Omit<ModelProfileUpsertRequestDto, "profile_id" | "api_key"> & {
  profile_id: string | null;
  api_key: string;
};

const EMPTY_MODEL_PROFILE_FORM: ModelProfileFormState = {
  profile_id: null,
  label: "",
  provider: "openai",
  base_url: "",
  custom_endpoint: "",
  model_name: "",
  api_key_label: "",
  api_key: "",
  activate: true,
};

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
  const [error, setError] = useState<{
    message: string;
    details?: string | null;
    correlationId?: string | null;
  } | null>(null);
  const [saved, setSaved] = useState(false);
  const [isFindingLibreOffice, setIsFindingLibreOffice] = useState(false);
  const [libreOfficeMessage, setLibreOfficeMessage] = useState<string | null>(null);
  const [apiRuntimeStatus, setApiRuntimeStatus] =
    useState<ApiServerStatusDto | null>(null);

  const [modelProfileList, setModelProfileList] = useState<ModelProfileListDto>({
    profiles: [],
    max_profiles: 10,
  });
  const [modelProfileForm, setModelProfileForm] =
    useState<ModelProfileFormState>(EMPTY_MODEL_PROFILE_FORM);
  const [isModelProfileDialogOpen, setIsModelProfileDialogOpen] = useState(false);
  const [isSavingModelProfile, setIsSavingModelProfile] = useState(false);
  const [openAIModels, setOpenAIModels] = useState<ModelInfoDto[]>([]);
  const [isFetchingOpenAIModels, setIsFetchingOpenAIModels] = useState(false);
  const [modelListMessage, setModelListMessage] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setIsLoading(true);
    Promise.all([tauriClient.getAppSettings(), tauriClient.listModelProfiles()])
      .then(([s, profiles]) => {
        if (!cancelled) {
          setSettings(s);
          setModelProfileList(profiles);
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

  useEffect(() => {
    setOpenAIModels([]);
    setModelListMessage(null);
  }, [modelProfileForm.provider, modelProfileForm.base_url, modelProfileForm.custom_endpoint]);

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

  function openCreateModelProfileDialog() {
    setOpenAIModels([]);
    setModelListMessage(null);
    setModelProfileForm({
      ...EMPTY_MODEL_PROFILE_FORM,
      base_url: settings?.base_url ?? "",
      custom_endpoint: settings?.custom_endpoint ?? "",
    });
    setIsModelProfileDialogOpen(true);
  }

  function openEditModelProfileDialog(profile: ModelProfileDto) {
    setOpenAIModels([]);
    setModelListMessage(null);
    setModelProfileForm({
      profile_id: profile.profile_id,
      label: profile.label,
      provider: profile.provider,
      base_url: profile.base_url,
      custom_endpoint: profile.custom_endpoint,
      model_name: profile.model_name,
      api_key_label: profile.key_label ?? profile.label,
      api_key: "",
      activate: profile.is_active,
    });
    setIsModelProfileDialogOpen(true);
  }

  function closeModelProfileDialog() {
    setIsModelProfileDialogOpen(false);
    setModelProfileForm(EMPTY_MODEL_PROFILE_FORM);
    setOpenAIModels([]);
    setModelListMessage(null);
  }

  function updateModelProfileForm<K extends keyof ModelProfileFormState>(
    key: K,
    value: ModelProfileFormState[K],
  ) {
    setModelProfileForm((prev) => {
      const next = { ...prev, [key]: value };
      if (key === "provider" && value !== "openai") {
        setOpenAIModels([]);
        setModelListMessage(null);
      }
      return next;
    });
  }

  async function refreshSettingsAndProfiles(nextProfiles?: ModelProfileListDto) {
    const [updatedSettings, updatedProfiles] = await Promise.all([
      tauriClient.getAppSettings(),
      nextProfiles ? Promise.resolve(nextProfiles) : tauriClient.listModelProfiles(),
    ]);
    setSettings(updatedSettings);
    setModelProfileList(updatedProfiles);
  }

  async function handleSaveModelProfile() {
    if (!modelProfileForm.model_name.trim()) {
      setError({ message: "请填写 Model Name。" });
      return;
    }
    if (!modelProfileForm.profile_id && !modelProfileForm.api_key.trim()) {
      setError({ message: "新增模型配置时需要填写 API Key。" });
      return;
    }
    setIsSavingModelProfile(true);
    setError(null);
    try {
      const request: ModelProfileUpsertRequestDto = {
        profile_id: modelProfileForm.profile_id,
        label: modelProfileForm.label.trim(),
        provider: modelProfileForm.provider,
        base_url: modelProfileForm.base_url.trim(),
        custom_endpoint: modelProfileForm.custom_endpoint.trim(),
        model_name: modelProfileForm.model_name.trim(),
        api_key_label: modelProfileForm.api_key_label.trim(),
        api_key: modelProfileForm.api_key.trim() || null,
        activate: modelProfileForm.activate,
      };
      const profiles = await tauriClient.upsertModelProfile(request);
      await refreshSettingsAndProfiles(profiles);
      setIsModelProfileDialogOpen(false);
      setModelProfileForm(EMPTY_MODEL_PROFILE_FORM);
    } catch (e) {
      setError(extractError(e));
    } finally {
      setIsSavingModelProfile(false);
    }
  }

  async function handleActivateModelProfile(profile: ModelProfileDto) {
    setIsSavingModelProfile(true);
    setError(null);
    try {
      const profiles = await tauriClient.activateModelProfile(profile.profile_id);
      await refreshSettingsAndProfiles(profiles);
    } catch (e) {
      setError(extractError(e));
    } finally {
      setIsSavingModelProfile(false);
    }
  }

  async function handleDeleteModelProfile(profile: ModelProfileDto) {
    setIsSavingModelProfile(true);
    setError(null);
    try {
      const profiles = await tauriClient.deleteModelProfile(profile.profile_id);
      await refreshSettingsAndProfiles(profiles);
    } catch (e) {
      setError(extractError(e));
    } finally {
      setIsSavingModelProfile(false);
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

  async function handleFetchOpenAIModels() {
    if (!settings || modelProfileForm.provider !== "openai") return;
    setIsFetchingOpenAIModels(true);
    setError(null);
    setModelListMessage(null);
    try {
      const result = await tauriClient.listOpenAIModels({
        ...settings,
        model_provider: "openai",
        base_url: modelProfileForm.base_url,
        custom_endpoint: modelProfileForm.custom_endpoint,
        model_name: modelProfileForm.model_name,
      }, modelProfileForm.api_key.trim() || null, modelProfileForm.profile_id);
      setOpenAIModels(result.models);
      if (result.models.length === 0) {
        setModelListMessage("没有获取到可用模型，请检查 Base URL 或 API Key。");
      } else {
        setModelListMessage(`已获取 ${result.models.length} 个 OpenAI 模型。`);
        const currentModel = modelProfileForm.model_name.trim();
        const currentModelInList = result.models.some((model) => model.id === currentModel);
        if (!currentModel || !currentModelInList) {
          updateModelProfileForm("model_name", result.models[0].id);
        }
      }
    } catch (e) {
      setError(extractError(e));
    } finally {
      setIsFetchingOpenAIModels(false);
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
        {error ? <ErrorMessage title="设置加载失败" message={error.message} details={error.details} correlationId={error.correlationId} /> : null}
      </div>
    );
  }

  const activeModelProfile = modelProfileList.profiles.find((profile) => profile.is_active);
  const canAddModelProfile = modelProfileList.profiles.length < modelProfileList.max_profiles;
  const isOpenAIProvider = modelProfileForm.provider === "openai";
  const canFetchOpenAIModels =
    isOpenAIProvider && (!!modelProfileForm.api_key.trim() || !!modelProfileForm.profile_id);
  const selectedOpenAIModelValue =
    isOpenAIProvider && openAIModels.some((model) => model.id === modelProfileForm.model_name.trim())
      ? modelProfileForm.model_name.trim()
      : "";

  return (
    <div className="settings-list">
      {error ? (
        <ErrorMessage title="设置操作失败" message={error.message} details={error.details} correlationId={error.correlationId} />
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

      <section className="panel model-profile-panel">
        <div className="panel-header compact">
          <div>
            <h2>模型配置</h2>
            <p className="muted-copy">
              每个配置包含 Provider、Endpoint、Model Name 和 API Key，最多保存 10 个。
            </p>
          </div>
          <div className="model-profile-header-actions">
            <StatusBadge tone={activeModelProfile?.api_key_configured ? "success" : "warning"}>
              {activeModelProfile ? `当前：${activeModelProfile.label}` : "未配置"}
            </StatusBadge>
            <Button
              onClick={openCreateModelProfileDialog}
              disabled={!canAddModelProfile || isSavingModelProfile}
            >
              新增模型配置
            </Button>
          </div>
        </div>

        <div className="model-profile-summary">
          <span>{modelProfileList.profiles.length} / {modelProfileList.max_profiles} 个配置</span>
          {activeModelProfile ? (
            <span>{providerLabel(activeModelProfile.provider)} · {activeModelProfile.model_name}</span>
          ) : (
            <span>新增并启用一个配置后即可进行模型分析</span>
          )}
        </div>

        <div className="model-profile-list">
          {modelProfileList.profiles.length === 0 ? (
            <p className="model-profile-empty">尚未添加模型配置。</p>
          ) : (
            modelProfileList.profiles.map((profile) => (
              <div className="model-profile-item" data-active={profile.is_active} key={profile.profile_id}>
                <div className="model-profile-main">
                  <div className="model-profile-title-row">
                    <strong>{profile.label}</strong>
                    {profile.is_active ? <StatusBadge tone="success">已启用</StatusBadge> : null}
                    <StatusBadge tone={profile.api_key_configured ? "success" : "warning"}>
                      {profile.api_key_configured ? "密钥已配置" : "密钥未配置"}
                    </StatusBadge>
                  </div>
                  <div className="model-profile-meta">
                    <span>{providerLabel(profile.provider)}</span>
                    <span>{profile.model_name || "未填写模型"}</span>
                    <span>{profile.base_url || "默认 Base URL"}</span>
                    {profile.custom_endpoint ? <span>{profile.custom_endpoint}</span> : null}
                  </div>
                </div>
                <div className="model-profile-actions">
                  {!profile.is_active ? (
                    <Button
                      onClick={() => void handleActivateModelProfile(profile)}
                      disabled={isSavingModelProfile}
                    >
                      启用
                    </Button>
                  ) : null}
                  <Button
                    onClick={() => openEditModelProfileDialog(profile)}
                    disabled={isSavingModelProfile}
                  >
                    编辑
                  </Button>
                  <Button
                    onClick={() => void handleDeleteModelProfile(profile)}
                    disabled={isSavingModelProfile}
                  >
                    删除
                  </Button>
                </div>
              </div>
            ))
          )}
        </div>
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

      {isModelProfileDialogOpen ? (
        <div className="model-profile-dialog-backdrop" role="presentation">
          <div className="model-profile-dialog" role="dialog" aria-modal="true" aria-label="模型配置">
            <div className="model-profile-dialog-header">
              <div>
                <h2>{modelProfileForm.profile_id ? "编辑模型配置" : "新增模型配置"}</h2>
                <p className="muted-copy">模型字段和 API Key 一起保存，启用后用于模型分析。</p>
              </div>
              <button
                type="button"
                className="image-lightbox-close"
                onClick={closeModelProfileDialog}
                aria-label="关闭"
              >
                ×
              </button>
            </div>

            <div className="model-profile-form setting-fields">
              <label>
                <span>配置名称</span>
                <input
                  type="text"
                  placeholder="例如 kkcoder gpt-5.5"
                  value={modelProfileForm.label}
                  onChange={(e) => updateModelProfileForm("label", e.target.value)}
                />
              </label>
              <label>
                <span>Provider</span>
                <select
                  value={modelProfileForm.provider}
                  onChange={(e) => updateModelProfileForm("provider", e.target.value)}
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
                  value={modelProfileForm.base_url}
                  onChange={(e) => updateModelProfileForm("base_url", e.target.value)}
                />
              </label>
              <label>
                <span>自定义 Endpoint</span>
                <input
                  type="text"
                  placeholder="留空使用默认"
                  value={modelProfileForm.custom_endpoint}
                  onChange={(e) => updateModelProfileForm("custom_endpoint", e.target.value)}
                />
              </label>
              <label>
                <span>Model Name</span>
                <div className="model-name-row">
                  <input
                    type="text"
                    placeholder={isOpenAIProvider ? "例如 gpt-5.5" : "zai-org/GLM-4.6V"}
                    value={modelProfileForm.model_name}
                    onChange={(e) => updateModelProfileForm("model_name", e.target.value)}
                  />
                  {isOpenAIProvider ? (
                    <Button
                      onClick={() => void handleFetchOpenAIModels()}
                      disabled={isFetchingOpenAIModels || !canFetchOpenAIModels}
                      title={
                        canFetchOpenAIModels
                          ? "从当前 OpenAI Base URL 获取模型列表"
                          : "请填写 API Key，或编辑已有配置"
                      }
                    >
                      {isFetchingOpenAIModels ? "获取中" : "获取模型"}
                    </Button>
                  ) : null}
                </div>
                {isOpenAIProvider && openAIModels.length > 0 ? (
                  <select
                    className="model-list-select"
                    value={selectedOpenAIModelValue}
                    onChange={(e) => {
                      if (e.target.value) updateModelProfileForm("model_name", e.target.value);
                    }}
                  >
                    <option value="" disabled>
                      选择已获取的模型
                    </option>
                    {openAIModels.map((model) => (
                      <option key={model.id} value={model.id}>
                        {formatModelOptionLabel(model)}
                      </option>
                    ))}
                  </select>
                ) : null}
              </label>
              {isOpenAIProvider && modelListMessage ? (
                <p className="setting-message">{modelListMessage}</p>
              ) : null}
              <label>
                <span>API Key 名称</span>
                <input
                  type="text"
                  placeholder="例如 kkcoder key"
                  value={modelProfileForm.api_key_label}
                  onChange={(e) => updateModelProfileForm("api_key_label", e.target.value)}
                />
              </label>
              <label>
                <span>API Key</span>
                <input
                  type="password"
                  placeholder={modelProfileForm.profile_id ? "留空则继续使用原密钥" : "输入 API 密钥"}
                  value={modelProfileForm.api_key}
                  onChange={(e) => updateModelProfileForm("api_key", e.target.value)}
                />
              </label>
              <label className="model-profile-checkbox">
                <input
                  type="checkbox"
                  checked={modelProfileForm.activate}
                  onChange={(e) => updateModelProfileForm("activate", e.target.checked)}
                />
                <span>保存后启用此模型配置</span>
              </label>
            </div>

            <div className="model-profile-dialog-actions">
              <Button onClick={closeModelProfileDialog} disabled={isSavingModelProfile}>
                取消
              </Button>
              <Button
                variant="primary"
                onClick={() => void handleSaveModelProfile()}
                disabled={isSavingModelProfile}
              >
                {isSavingModelProfile ? "保存中" : "保存配置"}
              </Button>
            </div>
          </div>
        </div>
      ) : null}

      {/* Save button */}
      <div className="settings-actions">
        <Button variant="primary" onClick={handleSave} disabled={isSaving}>
          {isSaving ? "保存中" : saved ? "已保存" : "保存设置"}
        </Button>
      </div>
    </div>
  );
}

function formatModelOptionLabel(model: ModelInfoDto): string {
  const descriptor = model.display_name ?? model.owned_by;
  return descriptor && descriptor !== model.id ? `${model.id} - ${descriptor}` : model.id;
}

function providerLabel(provider: string): string {
  switch (provider) {
    case "siliconflow":
      return "硅基流动";
    case "mimo":
      return "MiMo";
    case "openai":
      return "OpenAI";
    case "anthropic":
      return "Anthropic";
    default:
      return provider || "Provider";
  }
}

function extractError(error: unknown): {
  message: string;
  details?: string | null;
  correlationId?: string | null;
} {
  if (typeof error === "object" && error !== null) {
    const e = error as Record<string, unknown>;
    const msg = typeof e.message === "string" ? e.message : null;
    const details = typeof e.details === "string" ? e.details : null;
    const cid = typeof e.correlation_id === "string" ? e.correlation_id : null;
    if (msg) return { message: msg, details, correlationId: cid };
  }
  if (error instanceof Error) return { message: error.message };
  if (typeof error === "string") return { message: error };
  return { message: "设置操作失败，请稍后重试。" };
}
