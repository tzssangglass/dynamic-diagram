# 布局引擎调研：统一尺寸、整体缩放与动画稳定性

需求更新：用户在初轮调研后提供 scatter-gather 截图，提出“画幅再扩大 1 倍”，暂按 2× 目标记录。画布排版空间与文字图标整体展示倍率的选择尚待明确。本文 1.5× 仍作为缩放公式的示例；它不是最新目标上限。布局库能力与三种尺寸分离的结论不受目标倍率变化影响。

调研日期：2026-09-20。范围：当前 Rust 渲染架构与官方库文档；这是方案建议，没有安装依赖、实现替换或跑库间性能比较。Taffy API 参考固定为本次查到的 0.14.0；项目现用 resvg/usvg 0.45.1。

**主推：保留 Rust 单 binary，以 Taffy 处理页面与复合节点的盒布局；项目自己的统一布局层负责文字测量、图形边界、连接锚点和动画布局计划；SVG 只消费完成布局的场景。** Graphviz/ELK 适合未来的拓扑自动布局，优先放在 authoring/compile 阶段生成坐标与 keyframes。

## 当前实现为何需要一个布局层

| 当前代码 | 实际机制 | 影响 |
| --- | --- | --- |
| [src/svg.rs](../src/svg.rs) 的 `W/H`、`HEADER_H/STAGE_H`、`sx/sy` | 页面固定 760×330；0..100 映射到固定 stage；图标和文字另用像素常数 | 改画布大小不会自动形成统一的元素比例、边距和文字预算 |
| `plan_node_layout` | 按附近图标中心和固定阈值选择标签在下/左/右 | 没有完整测量标签、badge、packet；也不能保证所有碰撞消失 |
| `status_badge`、`wrap` | 字符数量估算宽度，按字符预算和空格分行 | 字距、不同字体、长无空格 token 的实际边界没有统一依据 |
| [src/main.rs](../src/main.rs) 的 `raster` | `DDA_SCALE` 只乘最终 pixmap 尺寸与 raster transform，默认 3 | 它控制输出采样，不改变逻辑布局；kitty 的固定 `c/r` 还会独立决定显示占位 |
| `fontdb`、`shared_render` | 已复用字体数据库和 pixmap | 新方案应延续复用；不能把已经存在的缓存再宣称为新增收益 |

这些结论来自当前源码。图标资源自身的 24/960 坐标网格、路径控制点属于图形数据；应保留在资源/统一图标适配器中。需要消除的是布局决策中的散落尺寸与位置，而非禁止系统拥有主题默认值。

## 候选方案与取舍

| 方案 | 可复用能力 | 仍需项目负责 | 与本项目的契合度 |
| --- | --- | --- | --- |
| **A. Taffy + 统一测量/场景层（主推）** | Rust；CSS Block、Flexbox、Grid；树形节点的尺寸与位置；leaf measure callback；高层 API 管理树与缓存 | 字体 shaping/换行、图连线和端口、自由图形碰撞、叙事语义、跨帧稳定 | 同一 Rust binary 可直接集成。适合 header/stage/caption，以及 icon/label/status、行列、卡片等复合结构 |
| **B. Yoga + 同样的项目适配层** | 成熟 Flexbox 盒布局；measure function；增量 dirty 布局 | 基本与 A 相同；还需 C/C++ 构建与 Rust FFI 维护 | 已有 Yoga/C++ 平台时合理；当前纯 Rust 项目没有明显收益抵消集成成本 |
| **C. Graphviz/ELK + 复合节点测量，编译成时间线** | 图拓扑的节点摆放与边路由；Graphviz 提供 layered/force/radial 等算法；ELK Layered 提供端口、正交/直线/spline 与复合图 | 先给出真实节点/标签尺寸；仍需页面布局、字体一致性、叙事位置约束、跨帧布局策略 | 最适合关系图自动排布。作为默认逐帧引擎会扩大系统复杂度；放在生成阶段可保持播放器单 binary |

A 的库能力来自 [Taffy 官方仓库](https://github.com/DioxusLabs/taffy)与 [Taffy 架构文档](https://docs.rs/taffy/0.14.0/taffy/)；B 来自 [Yoga 架构说明](https://www.yogalayout.dev/docs/about-yoga)、[外部测量接入](https://www.yogalayout.dev/docs/advanced/external-layout-systems)和[增量布局](https://www.yogalayout.dev/docs/advanced/incremental-layout)；C 来自 [Graphviz 布局算法](https://graphviz.org/docs/layouts/)与 [ELK Layered](https://eclipse.dev/elk/reference/algorithms/org-eclipse-elk-layered.html)。表中契合度是结合项目约束的工程判断，不是这些库的性能排名。

### 不应误判库能力

- **Taffy/Yoga 是盒布局器，不是图布局器。** Taffy 的 parent/children 表示容器关系，不能把任意有向边直接作为布局树。测量回调让应用提供内容尺寸，并不附带文字排版引擎。Taffy 官方也明确其与 Yoga 的布局基准不包含文字排版或建树成本。[Taffy 官方说明](https://github.com/DioxusLabs/taffy)
- **Flex/Grid 不会自动避免任意覆盖物碰撞。** 自由坐标、悬浮标注、移动 packet 和跨节点边仍要由场景边界与碰撞规则处理。该限制是依据盒布局输入/输出模型推导出的适用边界；不应把引入依赖等同于“所有布局问题解决”。[Taffy 架构](https://docs.rs/taffy/0.14.0/taffy/)
- **ELK 更接近节点连线图的需求，但不是现成 Rust 渲染框架。** 官方 ELK 是 Java 系统，elkjs 的布局代码由 Java 经 GWT 生成 JavaScript。它不提供渲染/样式。部署时直接依赖 Java/Node 会改变当前分发方式；嵌入运行时也需额外工程。[elkjs 官方仓库](https://github.com/kieler/elkjs)
- **Graphviz 不必强制使用外部 `dot` 进程。** 官方支持库内 `gvLayout`，因此不能简单声称它与单 binary 原则绝对不相容；但 C 库、插件与字体等构建集成仍需专门验证。部分路由模式也有边界，例如官方说明 `splines=ortho` 当前不处理 ports，且在 dot 中不处理 edge labels。[Graphviz 库手册](https://graphviz.org/pdf/libguide.pdf)、[splines 文档](https://graphviz.org/docs/attrs/splines/)
- **重新排布后的确定性，不等于动画稳定性。** ELK 提供 model order、interactive 等控制，官方仍把基于旧布局的动态更新单列为 FAQ。固定 random seed 只能减少随机性，不能保证更换标签/增删节点时没有跳位。[ELK Layered 选项](https://eclipse.dev/elk/reference/algorithms/org-eclipse-elk-layered.html)、[elkjs 动态布局 FAQ](https://github.com/kieler/elkjs#faqs-and-recurring-issues)

### 原生 Rust 图布局的后续候选

存在可以核验源码与已发布 API 的候选，不能据此认定已经达到本项目所需的完整性：

- `layout-rs`：本次 API 文档版本为 0.1.3，支持构造有向图、节点尺寸、DOT 输入与 SVG 输出；官方明确不支持 nested graphs、embedded HTML。可对无嵌套图验证几何结果复用，但不能直接承担所需的分组拓扑与所有端口路由。[官方仓库](https://github.com/nadavrot/layout)、[0.1.3 API](https://docs.rs/layout-rs/0.1.3/layout/)
- `rust-sugiyama`：本次 API 文档版本为 0.4.0，实现分层图布局，提供带节点尺寸的输入；公开入口返回各连通分量的布局。可评估为纯 Rust 节点摆放候选，完整端口、复合图、标签避让、边路由和时序稳定仍需逐项验证，不能从“Sugiyama”这一算法名称推定已经具备。[官方仓库](https://github.com/paddison/rust-sugiyama)、[0.4.0 API](https://docs.rs/rust-sugiyama/0.4.0/rust_sugiyama/)

本轮只确认候选存在及公开能力边界，没有评估维护响应、执行其测试或比较本项目样例。因此当前不以其中任何一个替代主推方案，也不承诺一个 crate 解决完整图布局；若后续确需 runtime 原生拓扑布局，应以节点尺寸、分组、端口、路由、确定性和性能验收结果选择。

## 推荐的统一机制

以下模块边界为本项目设计建议，名称仅示意；不是已经存在的实现。

### 初期完整契约：布局意图、世界坐标与图拓扑

| 模式 | 作者声明什么 | 统一机制保证什么 | 明确的边界 |
| --- | --- | --- | --- |
| **结构布局：`flow` / `grid` / `groups`** | 流向、行列、父子分组、排列顺序、对齐、可用空间、必要的尺寸约束；边仍通过稳定节点 ID 引用 | TextMeasure 先提供真实尺寸，Taffy 求解容器和复合节点盒；组整体具有可测量边界，子元素位置由父容器派生 | 图中的边不会自动成为 Flex/Grid 约束；Taffy 不会根据依赖边自动分层或消除交叉 |
| **固定世界坐标：`world`** | 地图、波形、显式轨迹及需要空间语义的节点坐标/关键帧；坐标空间和 fit/overflow 策略 | 保留坐标的叙事含义；只通过统一 world→stage transform 映射，配合同一文字测量、节点边界和越界诊断 | 不隐式挪动作者的世界坐标来消除碰撞；允许的放大、留白、裁切或拒绝由统一策略决定，默认报告未解决冲突 |
| **任意图拓扑：后续 `graph` adapter** | 节点、边、方向、分组/端口等图约束，不提供全部位置 | 在生成/编译阶段把已测量的节点/组尺寸交给经验证的 Graphviz/ELK adapter，输出坐标与边路径，再编译为数据/keyframes | 初期不宣称实现任意图自动摆放或避障路由；原生 Rust 候选须通过同一契约验收后才能加入 |

三者可以组合：例如页面由 flow 排出 header/stage/caption，stage 内某组用 grid，另一个区域保留世界坐标；后续 graph adapter 把已完成内部布局的组视为有尺寸的节点。**父子容器树与节点连接图是两个不同模型**，统一的是测量、坐标、锚点、输出与校验契约。初期已有 links 从求解后的节点边界取得端点；作者指定的路径仍受边界检查，不能将端点吸附误报为自动路由完成。

这些意图先作为 authoring/spec sugar 的声明式输入，由通用编译/布局步骤展开；timeline 仍保存 keyframes，不加入表达式执行或每个动画专属 Rust。可选布局字段、冲突诊断和能力声明应写入正式 spec 契约，并以真实样例验收。只有旧坐标图得到完整的尺寸检查，还不能称作 flow/grid 自动布局已经交付。

```text
Doc / keyframes
    ↓ 文档准备：收集稳定节点 ID、文字状态、约束与布局阶段
LayoutPlan（可跨帧复用）
    ↓ frame_at(t) + 布局约束
TextMeasure → 页面/复合节点盒布局 → 锚点/路由/边界检查
    ↓
ResolvedScene（逻辑单位的矩形、基线、路径、图标变换）
    ↓
SVG → raster sampling → PNG / kitty placement
```

1. **主题与布局输入统一。** 把字体样式、间距等级、描边、图标目标盒和内容边距收敛到少量主题 token；组件内部从 token 和测量结果派生，不再散落 `+30`、`+40`、`W-140`。页面盒布局负责 header、stage、caption 的真实尺寸；场景 0..100 坐标由一个 stage transform 转换。
2. **文字只由一个测量服务解释。** 输入包括字体标识/实际 font face、字号、字距、文本、宽度约束和换行策略；输出包含 advance、baseline、行盒与可见边界。badge 的外框来自测量宽高加 padding；caption 的分行预算来自可用宽度。缺字处理也归同一服务，避免“测量用一种字体、raster 用另一种”。
3. **Taffy 负责可表达为盒模型的部分。** 先把页面、node 的 icon/label/status、packet 的 label/padding 接入。既有显式动画坐标继续作为 stage 内锚点；用 Taffy 计算复合节点尺寸，不强迫地图、波形、自由路径变成 Flex 流。测量服务经 `compute_layout_with_measure` 接入，接收约束并返回测量结果。[Taffy 0.14.0 API](https://docs.rs/taffy/0.14.0/taffy/struct.TaffyTree.html#method.compute_layout_with_measure)
4. **场景几何与 SVG 发射解耦。** 保存完整节点占位框、标签/badge 框、端口和边的路径；碰撞诊断和 SVG 使用同一份结果。连接线以形状边界锚点为依据；路径和 packet 进度必须共享几何，避免放大节点后线端仍留在旧位置。
5. **生命周期稳定。** 当前 `Frame` 缺少节点 ID，单帧也看不到未来状态；稳定布局计划应在 `Doc` 准备阶段保留 ID，并收集字符串 keyframe 的尺寸包络。可为状态文字保留该阶段最大槽位，禁止数值变化时每帧左右翻转。允许拓扑变化的文档显式划分布局阶段；阶段间变化预计算为 keyframes。不能只用“上一帧位置”作隐含输入，否则随机访问 `frame_at(t)` 的结果可能依赖播放顺序。
6. **可行性检查先于视觉补丁。** 固定坐标之间若空间不足，统一规则应报告“哪个元素、需要多大、违反哪个边界”，或在文档允许时扩大 stage/重排。对约束互相冲突的输入，任何库都不应承诺绝对无碰撞。

文字服务的短期实现可先复用已经安装的 usvg 字体解析与文字布局来建立测量基准，并缓存唯一文字状态；不必立刻加入另一套 shaping 引擎。usvg 0.45.1 会解析文字 chunks/spans，并公开文字 layout；本机该版本官方源码的 `Text::bounding_box` 特别说明 SVG 文字包围框不等于紧致可见轮廓，因此必须明确使用行盒、advance 还是 ink bounds。[usvg 官方文档](https://docs.rs/usvg/0.45.1/usvg/)、[本机 usvg 官方源码](/home/kurt/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/usvg-0.45.1/src/tree/text.rs:585)

如果后续改用独立 shaping/line-breaking 库，应验证它与 usvg 字体选择、字距、fallback 和输出的对应关系；换库本身不能保证测量与绘制一致。对每个候选换行字符串都重新解析 SVG 的实现也必须缓存和测量成本，避免把旧的宽度估算问题换成新的热路径。

## 把“整体 1.5 倍”与清晰度分开

SVG `viewBox` 描述逻辑用户空间，外层 viewport 的 `width/height` 决定其映射，`preserveAspectRatio` 控制等比适配；标准给出了映射的 translate/scale 计算规则。因此整个画面的放大应只发生在统一输出变换/viewport，不应逐个乘坐标、字号和图标路径。[SVG 2 坐标规范](https://www.w3.org/TR/SVG2/coords.html#ViewBoxAttribute)

建议显式定义三个量：

| 参数语义 | 控制什么 | 推荐关系 |
| --- | --- | --- |
| `logical_size` | 排版可用空间与长宽比 | 默认从现有 760×330 兼容起步；内容重排时才改变 |
| `presentation_scale` | 用户看到的整体大小 | 支持 1.0、1.5 等；文字、图标、描边、留白一起缩放 |
| `raster_scale` | 每个显示单位使用多少采样像素 | 与展示倍率独立；当前 `DDA_SCALE` 对应这一项 |

计算关系：`display_size = logical_size × presentation_scale`，`raster_size = ceil(display_size × raster_scale)`。例如逻辑 760×330、展示 1.5、采样 1 时，SVG viewport/PNG 为 1140×495，viewBox 仍是 `0 0 760 330`。采样 3 时 PNG 为 3420×1485，但同一个显示容器不因此占据更多屏幕空间。这些数字用于说明既有基线的统一倍率，不是新的分散布局常数。

kitty 显示必须一起消费 display size；它已经通过 `c/r` 显式指定终端占位，单独增加 PNG 分辨率不会放大画面。终端若已占满可用宽度，1.5 倍会受空间限制：应使用统一 fit 规则输出实际倍率，不能把“更大采样 PNG”报告为已显示 1.5 倍。浏览器/图片宿主若另行施加宽度约束，也会影响最终物理大小。

另一个已核对的宿主约束是 [pi-diagram/extensions/index.ts](/home/kurt/workspace/my-dev/pi-diagram/extensions/index.ts:123) 的动画 `Image` 固定 `maxWidthCells: 84`，且动画采样通过 `DD_ANIM_SCALE` 传给 `DDA_SCALE`。**84→126 只是“当前上限 × 1.5”的算术示例，126 不应成为新常数，也不保证实际显示宽度扩大 1.5 倍。** 应由共享展示策略/尺寸元数据与实际终端可用 cells 派生 `maxWidthCells`；实际占位受目标显示尺寸、容器宽度和单元格尺寸共同约束。像素到 cells 的转换优先使用宿主实际单元格指标，必要的兼容回退也应集中配置，不能把注释中的约略 px/cell 比率复制到新布局代码。该相邻项目本轮仅只读核对。

纯整体缩放不会修复已有重叠；如果目标是增加内容可用空间，应改变 `logical_size` 并触发布局，而不是混淆两个参数。

## 性能方向与可验证标准

这些是待测假设，不是已证明的加速结论。官方 Taffy/Yoga 布局基准不能直接代表当前包含 SVG 解析、raster、PNG 编码与 kitty 输出的流水线。

- 分段记录 `frame_at`、测量/布局、SVG 生成、usvg parse、raster、PNG encode、传输耗时，并区分首帧与稳定循环。先确定占比再决定缓存层级。
- 缓存文字状态与布局计划，保留 Taffy 树及其有效缓存；packet 仅位置进度变化时，应避免重新测量静态节点。缓存 key 要包括实际字体、字体样式、文本、宽度约束和主题版本。
- 大小扩大 1.5 后，同采样倍率像素数量是原来的 2.25 倍；这是几何关系，不代表整帧耗时恰好增加 2.25 倍。采样质量应通过渲染对照与实际终端尺寸选择，不能用降低画质掩盖布局开销。
- 对背景缓存、已解析图标片段或完整 SVG tree 缓存逐个实验。usvg 是强类型已解析树，但当前每帧完整 XML 输入仍会产生解析工作；能否安全复用并局部更新要单独验证 API 与视觉语义。[usvg 解析模型](https://docs.rs/usvg/0.45.1/usvg/)

验收应包含：28 个参考 sim 的既有 `check`；长 label/status/caption 和无空格长 token；密集节点；低部节点与 caption 边界；统一 1.0/1.5 倍；`frame_at(t)` 顺序播放和随机访问结果一致；使用实际渲染边界进行检查，再人工查看关键 PNG。性能报告需同时给出配置、图像分辨率、首次/重复帧、p50/p95 与必要的内存数据，且只比较相同质量/相同工作量的配置。

## 推荐落地顺序

先建立统一主题、文字测量、逻辑/展示/采样坐标契约与 `ResolvedScene`，把当前 SVG 里的布局决定搬到一个可检查的结果模型；随后接入 Taffy 的页面与复合组件布局，保留手工关键帧的空间意图；最后在确有自动拓扑排布需求时，以已测量节点尺寸接 Graphviz/ELK 的生成阶段适配器。所有自动计算都落入通用机制或编译产物，新增 sim 仍然只是数据，timeline 文档仍只有 keyframes。
