# Sim 库策略调查：要么没有，要么全有？

**问题**：dynamic-diagram 的目标是覆盖整个软件工程乃至真实世界的运转机制。28 个 sim 显然"不够"——那么 sim 库应该消失（纯生成机制），还是应该穷举（"所有的 sim"）？

## 调查发现

### 1. 世上不存在"全有"——最优案例也只有 24~100 个，且全是手工

| 系统 | 规模 | 性质 |
|------|------|------|
| VisuAlgo（NUS，算法可视化标杆，2011 年至今） | **~24 个模块** | 教授团队十余年手工维护的 JS |
| Explorable Explanations 目录（explorabl.es，全品类交互解释目录） | 全网汇总 **~100+** | 每个都是独立作者手写的 bespoke 代码 |
| Mermaid | ~20 种图类型 + 精选示例页 | 语法是产品，示例只是教学 |
| 本项目 28 sims | 28 | 从 fazamhd 移植的 faithful ports |

"软件工程 + 真实世界机制"的空间无界；全世界最好的策展也只到 ~100，且无一来自数据吸收，全部手工/移植。**"有所有的 sim"作为目标在现实中不存在先例。**

### 2. 不存在可吸收的上游语料（"Material icons 时刻"没有发生）

- 图标能到 11,831 个，是因为 Google/Tabler/云厂商把图标**作为结构化数据发布**（Apache 2.0 的 path 表）
- 机制动画没有对应物：VisuAlgo 是 bespoke JS；explorables 是 bespoke 代码；学术界最接近的是 JAAL（JSON 格式的算法动画答题记录，面向 OpenDSA 教学记录，非可复用叙事库）
- 结论：无法像吞图标一样"吞"一个动画语料库

### 3. 规模化的现实路径是生成，且正在发生

- LLM→解释动画已是一个活跃社区：manim-mcp（带 5,300+ 文档索引的 MCP server）、LLM2Manim、Generative Manim、manimator（论文→动画）等多个开源项目
- 它们全部走**代码生成**路线（生成 Manim 脚本）——错误率高、不可审查是公认痛点
- 本项目的 timeline JSON 是**声明式数据**（无表达式、可 diff、可校验）——比代码生成更适合 AI 产出，这是差异化机会

## 结论：二选一是假问题，现实给出第三选项

每个成功系统都选了同一条路：

> **语法是产品，示例只是种子；规模来自生成。**

- Mermaid 的价值不在示例库，在语法 + Live Editor
- VisuAlgo 的 24 个是教学内容的本体（人家产品就是课件），不是"库覆盖"
- 本项目的对应物：**机制（timeline）是产品，28 个 sim 降格为种子与回归语料，规模靠 pattern 层 + AI 生成**

## 建议

1. **重新定位 28 sims**：不叫"库"，叫 **seed corpus**——双重身份：check 的回归语料 + AI 生成的 few-shot 范例（教风格）。不再追求数量覆盖。
2. **补 pattern 层**（上轮方案）：6-8 个叙事模板编译成 timeline——"挑动画像挑图标"，这是把 11,831 图标的世界观对齐到动画侧的正确方式。
3. **可选的规模化通道**（"sim garden"）：AI 按 pattern + 参照 seeds 批量生成新机制动画 → 人审 → 以 JSON 提交。数据化让"加一个 sim"从写代码变成审数据。是否现在投入取决于你是否要主动扩内容。

## 决策点

- A. 接受"种子+生成"定位（推荐）：开工 pattern 层
- B. 仍要扩量：pattern 层 + AI 批量生成首轮 50-100 个软件工程机制（garden 流程）
- C. 激进：删掉 sims（纯机制）——不推荐，失去回归语料与 few-shot 范例，违背 faithful-port 的既有投入

## 来源

- VisuAlgo 规模与性质: https://visualgo.net/en , https://visualgo.net/statistics
- Explorable Explanations 目录: https://explorabl.es/all/ , https://ncase.me/projects/
- Mermaid 示例与语法分野: https://mermaid.js.org/syntax/examples.html , https://github.com/mermaid-js/mermaid/blob/develop/packages/mermaid/src/docs/syntax/examples.md
- JAAL（JSON 动画格式、非语料库）: https://github.com/Aalto-LeTech/JAAL
- LLM→Manim 生成社区: https://github.com/paulnegz/manim-mcp/ , https://github.com/dineshmc1/generative-manim
