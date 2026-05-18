import { Button } from "../../components/common/Button";
import { EmptyState } from "../../components/common/EmptyState";
import { StatusBadge } from "../../components/common/StatusBadge";

export function SearchPage() {
  return (
    <div className="page-grid search-layout">
      <section className="panel panel-wide">
        <div className="panel-header">
          <div>
            <p className="eyebrow">查询</p>
            <h2>搜索页面</h2>
          </div>
          <StatusBadge>索引未建立</StatusBadge>
        </div>
        <div className="search-bar" aria-label="搜索输入占位">
          <input disabled placeholder="输入关键词后，后续版本会查询本地索引" />
          <Button variant="primary" disabled>
            搜索
          </Button>
        </div>
      </section>

      <section className="panel">
        <div className="panel-header compact">
          <h3>结果列表</h3>
          <StatusBadge>空</StatusBadge>
        </div>
        <EmptyState title="暂无搜索结果" description="索引服务接入后，这里会展示页面命中、来源文档和置信信息。" />
      </section>

      <section className="panel">
        <div className="panel-header compact">
          <h3>图片预览</h3>
          <StatusBadge>待选择</StatusBadge>
        </div>
        <p className="muted-copy">选择搜索结果后，页面图片预览会显示在这里。</p>
      </section>

      <section className="panel">
        <div className="panel-header compact">
          <h3>JSON 查看</h3>
          <StatusBadge>待接入</StatusBadge>
        </div>
        <pre className="json-placeholder">{`{
  "status": "功能待接入",
  "workspace": null
}`}</pre>
      </section>
    </div>
  );
}
