# 统一布局、放大与性能：实现验收

2026-09-20。基于 `ad20a6f` 的工作区改动；关联扩展位于 `../pi-diagram`。
延续 pi 会话 `01a0afb6-c0fc-7091-b033-5a0ccfac2037` 的分析及用户批准的实现。

## 实现

- `layout.rs` 集中 canvas、字体尺寸、间距、圆角、线宽等主题参数；
  Taffy 负责节点组合盒、flow/grid/columns 和页面分区。图标自身的路径坐标
  是资源数据，实际图标尺寸统一来自主题。
- `typography.rs` 用与 resvg 相同的字体库测量并缓存文字，按实际宽度换行。
  `Renderer` 按完整时间线准备布局，状态和可见性变化不会重新排版。
- 固定坐标保留相对位置意图；过密的同排标签自动换行，不可满足的约束给出
  错误。结构布局可省略坐标，增加高度后由 Taffy 分配行间空间。
- ID 连线和移动数据包共享节点边界路径。短路径的标签移至附近空位，通过
  引线标记真实位置，避免因节点避让而整段消失。
- `canvas.scale`/`--scale` 支持整体 1.5×、2× 展示；`min_height`/`--height`
  增加逻辑空间，`--density` 独立控制 PNG 采样。终端/浏览器按宿主空间适配。
- 渲染器复用像素缓冲，对不透明图像省去拷贝/反预乘；相同 SVG 的 PNG 缓存
  限制为 16 MiB/64 项，单张图限制为 32 Mi 像素。
- pi 生成改为可取消的异步子进程，支持超时及强制终止；动画按 fps/时长采样
  （默认 24fps、最多 240 帧），按真实经过时间播放。首帧复用为海报；唯一
  临时目录和 manifest 隔离导出。base64 缓存上限默认 16 MiB，统一调度器及
  会话清理限制生命周期；内联动画使用宿主宽度，移除 84 列上限。

## 图像

| 场景 | 旧逻辑尺寸 | 新逻辑尺寸 | 复核图 |
| --- | --- | --- | --- |
| 用户的 scatter-gather | 760×330 | 760×684（高度 2.07×） | [PNG](scatter-gather.png) |
| CI pipeline | 760×330 | 760×608 | [静态](ci-pipeline.png)、[移动标签](ci-moving.png) |
| 机制说明 | 760×330 | 760×622 | [PNG](mechanism.png) |
| 无坐标布局 | — | 760×660 | [示例](../../examples/auto-layout.json)、[PNG](auto-layout.png) |
| TCP 握手种子 | — | 内容驱动 | [PNG](tcphs.png) |

上述 PNG 以 density=2 生成。用户原来的 `.diagrams/scatter-gather.png` 也已
用新引擎重新生成（默认 density=3）。已有 pi 历史消息携带的图片不会自动
替换；`/reload` 后重新调用工具可使用新扩展。

## 性能

同机 release，Ryzen AI 9 HX PRO 370。比较相同新 SVG、相同像素尺寸，参考
路径为原来的 usvg/resvg + tiny-skia PNG 编码；优化路径为生产 Rasterizer。
两者均包含 SVG 输出和 base64，均复用字体库及像素缓冲。每项记录 96 个样本，
首轮采样前预热 8 帧，重复循环前预热 24 帧。没有计入文件写入、终端传输或
一次性布局准备。不同时间可能对应完全相同画面，首轮结果也可能命中缓存。

| 场景 / density | 像素尺寸 | 首轮参考 p50 | 首轮优化 p50 | 首轮优化 p95 | 24 帧循环优化 p50 |
| --- | --- | ---: | ---: | ---: | ---: |
| CI / 2 | 1520×1216 | 7.22 ms | 6.32 ms | 6.58 ms | 0.188 ms |
| CI / 3 | 2280×1824 | 12.37 ms | 9.74 ms | 10.75 ms | 0.306 ms |
| 机制 / 2 | 1520×1244 | 7.41 ms | 6.32 ms | 6.66 ms | 0.181 ms |
| 机制 / 3 | 2280×1866 | 13.19 ms | 10.28 ms | 11.30 ms | 0.293 ms |

字体库预热后的首次布局准备约 5–12 ms，按文档支付一次。重复 24 帧工作集
适合缓存；这些收益不能外推为所有实时新帧都只需 0.3 ms。完整原始结果在
[benchmarks.json](benchmarks.json)，复现：

```sh
cargo build --release
python3 outputs/unified-layout/benchmark.py
```

真实 CLI 导出 CI 的 24 帧，包含进程启动、布局、PNG 和写文件，6 次运行取
后 5 次中位数。新图高 608，旧图高 330；下表是实际任务对比，不是同图对照：

| density | 历史基线 | 本次 | 变化 |
| --- | ---: | ---: | ---: |
| 2 | 162 ms | 134 ms | -17% |
| 3 | 264 ms | 222 ms | -16% |
| 4.5 | 426 ms | 371 ms | -13% |

数据在 [cli-benchmarks.json](cli-benchmarks.json)，历史数据在
[基线目录](../layout-performance-audit/)。未清空操作系统文件缓存，因此不能
作为冷磁盘测试。更大图片仍会增加终端传输和显示成本，本次没有声称降低
宿主 RSS 或测得终端端到端帧率。

## 验证

- `cargo build --release`：通过，无警告。
- `cargo test --release`：23 个核心测试 + 6 个 CLI 测试通过。
- `dynamic-diagram check`：28 个数据动画的语义回归通过；原有 152 个语义断言
  保留，旧固定尺寸相关检查由实际 usvg 文字边界测试替代。
- 新测试覆盖全文档稳定布局、完整种子库多个时间点、长标签、长字幕、移动
  注释、无坐标异构节点、残缺列顺序、2× 缩放、真实路由/数据包、无效参数、
  像素和缓存预算、透明缓冲清理、低帧率 Ctrl+C 响应。
- pi `npm test` 及 `DYNAMIC_DIAGRAM_TEST_BIN=… npm test`：通过；覆盖异步性、
  取消/超时、manifest、帧数上限、保持周期、宽度适配、缓存和退出清理，以及
  真实引擎静态 PNG/动画/2× 展示。
- 独立审查发现低帧率等待问题，已修复并加回归；图片复核另发现残缺列和
  短路径标签问题，已补充失败测试后修复。

## 使用与边界

```sh
dynamic-diagram spec scene.json png --scale 2 --density 2
dynamic-diagram spec scene.json png --height 660
dynamic-diagram spec scene.json info
```

宿主宽度有限时会等比适配，`scale=2` 不保证物理屏幕宽度增加两倍。
字体保留 Inconsolata/Liberation Sans，不包含完整 CJK/emoji 字形。结构布局
解决盒排版，不是自动图拓扑布线；原始 v2 路径和有意的连线交叉仍由作者控制。
极密场景没有可用标签空位时可能省略标签，需要增加空间或调整结构。

本次使用已搜索的性能 skill 工作流，以及 codebase-design、项目
dynamic-diagram、writing-for-agents、requesting-code-review 和
verification-before-completion；来源和前期调研保留在
[分析文档](../layout-performance-analysis.md)。
