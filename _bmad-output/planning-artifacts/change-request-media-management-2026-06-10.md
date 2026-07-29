---
source: user-request
date: 2026-06-10
status: captured
---

# Media Management And Workbench Routing Change Request

## User Requirements

1. Rename the left sidebar entry `图片导入` to `媒体导入`.
2. Add a `媒体管理` feature tab, and move the document-management module currently shown in the workbench into the new media-management tab.
3. When the user wants to re-analyze one image/media item or a batch of items, clicking `重分析` should route to the original model-analysis module. The user can either add a custom prompt for large-model re-analysis or directly modify the JSON for fine-tuning.
4. The workbench tab should only display status and route users to the correct feature tabs. Concrete operations should happen in the corresponding feature tabs.
