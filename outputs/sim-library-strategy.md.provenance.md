# Provenance — sim-library-strategy.md

调研日期: 2026-09-19 · 方法: 两轮多角度 web 检索（pi web_search），无页面级全文抓取

## 逐条证据与可信度

| 论断 | 来源 | 可信度 | 备注 |
|------|------|--------|------|
| VisuAlgo ~24 个模块 | visualgo.net/en, /statistics 快照 | 中 | 数字在多个快照重复出现，可能同源；量级（十位数量级）高置信 |
| explorable.explanations 目录 ~100+ | explorabl.es/all/, ncase.me/projects/ | 低-中 | 搜索摘要称"100+"，未逐条点数；量级（百位数量级）中置信 |
| Mermaid 语法为产品、示例为精选 | mermaid.js.org 示例页结构 | 高 | 官方文档结构直接可见 |
| JAAL 是答题记录格式而非可复用叙事库 | github.com/Aalto-LeTech/JAAL | 中 | 仓库 README 描述；未审计其数据内容 |
| LLM→Manim 社区活跃（≥4 项目） | manim-mcp, generative-manim 等仓库 | 中 | 均为社区项目，成熟度未独立验证 |
| 不存在可吸收的机制动画结构化语料 | 检索 absence | 中 | 阴性结论，基于两轮检索未命中；无法穷尽证明 |

## 局限

- 未做 VisuAlgo/探索目录的逐项清点（量级足够支撑结论，精确数不影响决策）
- 未评估各 LLM→Manim 项目的实际产出质量（与本项目路线对比仅到"代码生成 vs 声明式"的定性层面）
