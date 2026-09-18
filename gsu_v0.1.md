# GSU v0.1 开发计划文档

版本：1.0  
日期：2026-09-18  
适用对象：项目负责人、Rust 开发者、测试与发布负责人  
交付目标：可独立安装和运行的 PyTorch 性能静态检查 CLI

本计划将 GSU v0.1 固定为“读取源码、识别检查点、定位并解释”的工具。开发完成后，用户无需修改 Python 项目，无需安装 PyTorch 或 CUDA，即可通过 `gsu check .` 获得 console 或 JSON 诊断。所有优化建议由用户评估和实施。

本文中的“必须”是发布验收要求；“建议”是默认工程方案；标记为“目标”的性能、版本和工期需由实际验证确认，不代表已实现能力。产品范围已冻结；本文补齐的参数语义、分析边界和资源限制作为开发基线，由对应任务验收，不隐含扩大产品范围。

## 目录

1. 产品定义与范围
2. 用户体验与交付原则
3. 技术选型与架构
4. 模块职责与目录结构
5. 核心数据模型
6. Parser 与轻量语义分析
7. 规则系统与公共约定
8. 八条规则详细规格
9. CLI 规范
10. 配置与文件发现
11. 输出与 JSON Schema
12. 错误处理与退出码
13. 性能与缓存
14. 测试与质量门禁
15. CI CD 与发布
16. 兼容性与安全约束
17. 里程碑与执行任务
18. 风险与缓解
19. v0.2 扩展边界
20. 发布核对清单与参考资料

## 1 产品定义与范围

### 1.1 产品定义

GSU 是 standalone CLI，产品形态参考 Ruff：从外部扫描文件或目录，使用稳定规则编号报告源码诊断。GSU 面向 PyTorch 代码中与设备传输、同步、设备选择和 dtype 转换有关的潜在性能问题。

GSU v0.1 不测量 GPU 利用率、传输耗时、模型吞吐或数值精度损失。诊断表示“静态证据支持用户检查此处”，不表示运行时必然慢，也不保证修改后加速。

### 1.2 冻结的交付范围

| 项目 | v0.1 要求 |
| --- | --- |
| 入口 | 单一 `gsu` 可执行程序 |
| 命令 | `gsu check`、`gsu rule`、`--help`、`--version` |
| 实现 | 主程序 Rust，生产运行不依赖 Python 解释器 |
| 分析 | Python AST、源码位置、轻量语义、静态规则 |
| 框架 | PyTorch 源码模式，设备分析聚焦 CPU 与 CUDA |
| 规则 | T001、T002、S001、S002、D001、D002、P001、P002 |
| 输出 | console 与 JSON |
| 配置 | `pyproject.toml` 的 `[tool.gsu]`，select、ignore、exclude |
| 退出码 | 0 检查完成无诊断；1 检查完成有诊断；2 检查未完整完成或执行失败 |
| 诊断维度 | Severity 与 Confidence 独立 |

### 1.3 目标

- 无侵入接入：不要求修改源码、添加 import 或初始化逻辑。
- 清晰定位：每条诊断关联文件、行列、源码范围与具体静态证据。
- 可解释：说明为什么值得检查、什么情况下是合理代码，以及人工建议。
- 可自动化消费：JSON 有版本和 schema，输出稳定，退出码适用于 CI。
- 保持小而快：单进程离线检查，有限依赖，不预建复杂平台。
- 控制误报：无法确认对象来源时不凭方法名猜测 Tensor。

### 1.4 非目标

不实现 `import gsu`、公开 Python SDK、装饰器、context manager、运行用户程序、runtime profiler、GPU 探测、`gsu run`、`gsu doctor`、`--fix`、自动 patch、格式化、自动重构、Web、数据库、LLM、JAX、TensorFlow、ROCm、硬件参数库、模型 accuracy 评估、量化、编译器、kernel 优化或分布式性能归因。

不建立 Fix、Edit、Patch、Replacement 等自动修改数据模型。不预留运行时插件、框架 backend 或硬件数据库接口。Rust 内部模块可以复用，但不承诺稳定公共库 API。

### 1.5 原始设计到当前范围的映射

上传的 `vibe_design.md` 强调精度、重复 copy、设备不统一，以及简约、小、快。当前将其收敛为 P 类 dtype 检查、T 类传输检查、D 类设备检查，并加入 S 类同步检查。原文件中 Python/Rust/C++/CUDA 的探索性选择，以当前 Rust CLI 方案为准；JAX/TensorFlow 的早期远景不进入本期。

## 2 用户体验与交付原则

### 2.1 典型流程

```bash
gsu --version
gsu check .
gsu rule T001
gsu check src/train.py --select T,S
gsu check . --output-format json > gsu-report.json
```

用户读取诊断、结合业务决定是否修改，再自行测试正确性与性能。GSU 不修改项目，也不执行用户的验证程序。

### 2.2 诊断表达原则

- 使用 potential、may、review、consider 等表述，不把可疑模式称为已证实瓶颈。
- 不报告估算加速倍数、浪费时间或运行次数；循环只说明语法上可能重复。
- high confidence 表示静态模式证据强，不表示性能影响已被测量。
- 同设备或同 dtype 转换可能直接返回原 Tensor；不得统一称为内存复制。
- 每批新数据的 CPU→GPU 传输可能是必要操作；不得统一建议移出循环。
- 精度建议必须保留用户验证数值稳定性与结果的责任，不直接要求 FP64 改 FP16。

CLI 的规则名称与诊断文案默认英文，本文说明中文。v0.1 不加入多语言切换，以减少快照和文案维护成本。

## 3 技术选型与架构

### 3.1 技术选型

建议采用单 Rust package、一个 binary target。锁定 `Cargo.lock`，采用 `rust-toolchain.toml` 固定编译器，第一周验证后写入明确版本号，禁止发布流程使用浮动工具链。

| 能力 | 建议依赖 | 决策与验收 |
| --- | --- | --- |
| CLI | clap | 参数校验、帮助与错误码覆盖 |
| Parser | rustpython-parser | 第一周验证 Python 3.10–3.12 目标语法与范围 |
| 配置 | serde、toml | 拒绝 `[tool.gsu]` 未知键 |
| JSON | serde_json | schema 与快照一致 |
| 文件发现 | ignore、globset | 显式配置遍历开关，避免环境默认差异 |
| 错误 | thiserror 或等价内部 enum | 错误码稳定、文案有上下文 |
| 测试 | insta、assert_cmd、tempfile 或等价方案 | 限 dev-dependencies |

`rustpython-parser` 提供 AST 解析能力；不要引入 RustPython VM。Parser 外包一层适配模块，隔离第三方 AST。官方 API 参考：[rustpython-parser](https://docs.rs/rustpython-parser/latest/rustpython_parser/)。

第一周必须形成 ADR-001：记录具体 crate 版本、许可证、MSRV、语法样例结果、位置正确性、发布体积与解析性能。若首选失败，仅在 Parser 适配层评估替代 Rust parser；不能以运行 Python AST 子进程替代 standalone 约束。不要直接将 Ruff 整个仓库纳入工程。

### 3.2 数据流

```text
CLI 参数
  → 配置解析与规则选择
  → 文件发现与过滤
  → 单文件读取与编码检查
  → Parser AST 与源码范围
  → 绑定解析与语义事实
  → 规则检查
  → 过滤 去重 稳定排序
  → Console 或 JSON
  → 退出码
```

单文件失败不阻止其他有效文件继续检查；全局配置失败则不启动扫描。规则没有文件写入、网络、进程调用或配置读取权限。

### 3.3 执行策略

先串行遍历和逐文件分析，分析完释放 AST、源码和语义状态，仅保留诊断所需的短摘录与结构化结果。文件列表和最终诊断排序后输出，确保文件系统顺序不影响结果。

v0.1 不要求线程池。若性能门禁失败，先剖析检查器自身并优化热点；增加文件级并行需要新 ADR、内存上限和确定性测试，不能通过降低检测覆盖率换取速度。

## 4 模块职责与目录结构

```text
gsu/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── README.md
├── LICENSE
├── CHANGELOG.md
├── src/
│   ├── main.rs
│   ├── cli.rs
│   ├── config.rs
│   ├── discovery.rs
│   ├── checker.rs
│   ├── source.rs
│   ├── parser.rs
│   ├── diagnostic.rs
│   ├── error.rs
│   ├── semantic/
│   │   ├── mod.rs
│   │   ├── bindings.rs
│   │   ├── facts.rs
│   │   └── calls.rs
│   ├── rules/
│   │   ├── mod.rs
│   │   ├── transfer.rs
│   │   ├── sync.rs
│   │   ├── device.rs
│   │   └── precision.rs
│   └── output/
│       ├── mod.rs
│       ├── console.rs
│       └── json.rs
├── schemas/diagnostics-v1.schema.json
├── tests/
│   ├── fixtures/{rules,semantic,parser,projects}/
│   ├── snapshots/
│   ├── cli.rs
│   ├── config.rs
│   ├── discovery.rs
│   └── schema.rs
├── benches/
├── docs/{rules,adr,release}/
└── .github/workflows/{ci,release}.yml
```

上面的花括号表示多个目录或文件，不是实际文件名。

| 模块 | 输入与输出 | 不承担的职责 |
| --- | --- | --- |
| main | 启动、错误收口、退出 | 不放检测逻辑 |
| cli/config | argv、TOML → EffectiveConfig | 不解析 Python |
| discovery | 根路径、排除项 → 有序文件与发现错误 | 不加载用户模块 |
| source/parser | 字节 → SourceFile、AST 或解析错误 | 不修复无效语法 |
| semantic | AST → 绑定、Tensor/device/dtype 事实、事件 | 不做完整类型推断 |
| rules | 事件、事实 → Diagnostic | 不直接打印 |
| checker | 编排、聚合、过滤、统计 | 不拼接具体规则文案 |
| output | RunReport → stdout/stderr | 不重新运行规则 |
| error | 内部异常 → 稳定操作错误 | 不混入规则编号 |

## 5 核心数据模型

以下 Rust 是实现形状示意，具体类型名可调整，但序列化契约按第 11 节执行。

```rust
enum RuleCode { T001, T002, S001, S002, D001, D002, P001, P002 }
enum Severity { Low, Medium, High }
enum Confidence { Low, Medium, High }

struct TextRange { start: u32, end: u32 } // UTF-8 字节，左闭右开
struct Position { line: u32, column: u32 } // 输出为 1-based
struct Location { path: String, start: Position, end: Position }
struct Evidence { kind: String, detail: String }

struct Diagnostic {
    rule: RuleCode,
    severity: Severity,
    confidence: Confidence,
    location: Location,
    range: TextRange,
    message: String,
    explanation: String,
    suggestion: String,
    evidence: Vec<Evidence>,
}

struct OperationError {
    code: String,
    message: String,
    path: Option<String>,
}

struct RunReport {
    schema_version: String,
    tool_version: String,
    complete: bool,
    diagnostics: Vec<Diagnostic>,
    errors: Vec<OperationError>,
    summary: Summary,
}
```

内部 SourceFile 持有原始源码、行起始偏移、路径和 AST 关联 ID。输出 JSON 不暴露内部 FileId、NodeId 或 parser 类型。

### 5.1 位置契约

- 内部 byte range 对应包含 BOM 时的原始 UTF-8 文件；去 BOM 解析必须加回偏移。
- 行从 1 开始；列按 Unicode scalar value 从 1 开始，结束位置排他；不是 UTF-16 code unit 或屏幕宽度。
- CRLF 按一处换行计算；tab 在位置中算一个字符，console 渲染按 4 列 tab stop 展开。
- console 下划线额外计算显示宽度，不能直接复用字节长度。
- JSON 路径为相对于项目根的 `/` 分隔路径；根外路径允许 `../`。不使用 basename 以免歧义。
- `range` 内部保留，JSON 初版只输出 location；集成端如需字节位置，走 schema 演进。

### 5.2 Severity 与 Confidence

Severity 表示该类模式成立时的潜在影响等级：low 为冗余表达或可配置性问题，medium 为可能的转换或同步成本，high 为循环内显式同步或明确往返模式的优先检查项。它不是错误等级，也不驱动不同退出码。

Confidence 表示证据强弱：high 为标准 PyTorch 名称解析和明确静态事实；medium 为已确认 Tensor 但 device、循环执行或目的尚不确定；low 保留于枚举，首批规则不使用纯方法名猜测来产生 low 诊断。

## 6 Parser 与轻量语义分析

### 6.1 输入边界

只分析 `.py` 普通文件，UTF-8 与 UTF-8 BOM。非 UTF-8 编码即使有 coding cookie，也返回 GSU-E005；不隐式转码。`.pyi`、notebook、压缩包、二进制扩展、stdin 输入不在本期。

Parser 按 module 模式处理。语法错误或不支持语法时，该文件整体不生成规则诊断；产生 GSU-E006，继续其他文件。禁止在损坏 AST 上输出看似完整的性能结论。

目标语法为 Python 3.10–3.12，不代表执行这些版本。使用 match、异常组、类型参数、f-string 变体等夹具核实覆盖；不能通过某个简单样例后宣称支持整个版本。更高版本能被解析也只视为未承诺兼容。

### 6.2 名称和作用域

必须支持标准导入及别名：

```python
import torch
import torch as t
from torch import tensor as make_tensor
from torch.cuda import synchronize as sync
```

解析为规范名，例如 `t.tensor` → `torch.tensor`。仅凭 `x.cuda()`、`value.item()` 不能确认 PyTorch 对象。

处理 module、function、class、lambda、comprehension 的作用域。函数局部绑定先收集：函数内任何赋值使名称成为局部，不能在赋值前误认为外层 import。方法体不能错误继承 class 局部变量作为词法作用域。

重赋值、参数同名、删除绑定后，相应 import/Tensor 事实失效。星号导入、动态 import、getattr、exec/eval、模块重导出、跨文件 import 的对象不解析；不执行任何一种表达式。global/nonlocal 涉及的写入在相关作用域内保守降级为 Unknown。

### 6.3 有限事实集合

```text
ObjectKind = TorchTensor | TorchModule | Other | Unknown
Device = Cpu | Cuda(index: Known(n) or Unspecified) | Unknown
Dtype = F64 | F32 | F16 | BF16 | OtherKnown | Unknown
ValueFact = kind + device + dtype + provenance + value_version
```

v0.1 必须内置的 Tensor 工厂：`torch.tensor`、`empty`、`zeros`、`ones`、`full`、`rand`、`randn`、`arange`、`as_tensor`、`from_numpy`。仅识别这些规范调用及其别名；`from_numpy` 可确认 Tensor，但不推断 ndarray 的 dtype。

不假定省略 device 或 dtype 的工厂一定是 CPU/float32，因为默认设置可能变化。显式字面量参数提供确定事实。`x: torch.Tensor` 形参视为 Tensor 声明证据，device/dtype 未知，confidence 上限 medium；普通未注解形参不猜测。

确认 Tensor 的 `.to`、`.cuda`、`.cpu`、`.float`、`.half`、`.double`、`.bfloat16`、`.detach`、`.clone` 返回值可继续跟踪；detach/clone 保留 device/dtype，但 clone 是新 value identity。未知算子、model 输出、容器下标、DataLoader 批次、复杂二元表达式不自动推断。TorchModule 不应用 Tensor 转换规则，避免混淆 Module.to 的原地语义。

### 6.4 device 与 dtype 解析

可解析常量字符串、整数索引、规范 `torch.device(...)`、局部单值变量和简单别名链。`torch.device("cuda", 0)` 与 `"cuda:0"` 规范化为同一固定索引；`"cuda"` 的当前设备语义不能等同于 `"cuda:0"`。

`.to` 必须区分 device、dtype、other Tensor 三种重载。支持明确的 `to("cuda")`、`to(device=d)`、`to(torch.float64)`、`to(dtype=dt)`、`to("cuda", torch.float32)`；`to(other)` 只有 other 的 Tensor 事实明确时才识别其 device/dtype。kwargs 展开、冲突参数或无法判别的动态实参降级为 Unknown，不猜测。

D002 比较转换时还必须检查 copy、memory_format、dtype、non_blocking。未识别的参数阻止“重复同等转换”结论。

### 6.5 轻量数据流

以函数或 module 内的顺序语句块为基本单元；仅跟踪局部名称和表达式链，不跟踪任意对象属性或堆别名。

1. 进入块时建立 BindingId 和 value version。
2. 赋值右侧解析后更新左侧事实；简单 `b = a` 可共享 provenance。
3. 分支分别分析，合流只保留双方完全相同的事实，否则 Unknown。
4. 循环体的局部赋值集合先收集：可能由上一轮改变的入口事实降级。循环内部按源码顺序跟踪，不跨迭代证明重复。
5. T002/D002 仅分析同一直线块或连续调用链；分支、循环边界、try/except、未知调用作为序列屏障。
6. 对未知函数调用，保守清除当前块的连续转换序列；传入或逃逸对象的 device/dtype 事实降级，Tensor 身份可保留。
7. 函数体单独分析；在 loop 中定义函数不等于函数被循环调用。lambda、生成器表达式不继承外层 loop 上下文。
8. for、while、async for 的主体标记可能重复；循环条件也处于重复上下文；for iterable 表达式不属于该循环的主体重复上下文。推导式只在其迭代部分内标记；不推断实际次数。

不做全程序 CFG、不做路径可达性证明、不做固定点跨循环推断、不做跨函数调用展开、不做字符串执行分析。`if False` 等死代码不额外优化；文档说明静态诊断可能包含不执行分支。

## 7 规则系统与公共约定

### 7.1 注册表

每条规则必须有唯一 code、稳定 name、简介、默认 Severity、触发条件、解释、建议、帮助文档与测试夹具。注册表同时驱动 select 展开、`gsu rule` 和有效规则验证。

| Code | Name | 默认 Severity | Confidence |
| --- | --- | --- | --- |
| T001 | transfer-in-loop | medium | medium/high |
| T002 | device-ping-pong | high | high |
| S001 | item-in-loop | medium | medium/high |
| S002 | explicit-sync | 循环内 high，否则 medium | high |
| D001 | hardcoded-device | low | high |
| D002 | repeated-device-cast | low | high |
| P001 | explicit-float64 | medium | high |
| P002 | repeated-dtype-cast | medium | medium/high |

前文历史讨论曾出现 S001/S002 示例错置；实现以此表为唯一编号映射：S001 是 item-in-loop，S002 是 explicit-sync。

### 7.2 规则接口

```rust
trait Rule {
    fn metadata(&self) -> &'static RuleMetadata;
    fn check(&self, event: &SemanticEvent,
             context: &SemanticContext,
             out: &mut Vec<Diagnostic>);
}
```

使用一次 AST 遍历生成按源码求值顺序排列的事件。事件至少含 call range、规范调用目标、接收者事实、转换前后事实、loop 上下文、序列屏障和相关前序调用位置。多个事件可以输出多条诊断；不要让返回一个 Option 限制序列型规则。

### 7.3 去重与覆盖

完全相同的 rule、path、start、end 只保留一条。不同规则可以并存，例如 `.double()` 在循环内可触发 P001/P002；同一表达式的 T001/D001 也可并存，因为关注点不同。不做依赖启用规则集合的跨规则抑制，确保 `--select` 不改变其他规则的语义。

T002 在往返闭合点报告一次，证据引用起点。D002 在第二个冗余转换调用报告。所有方法规则使用整个调用表达式范围；D001 使用索引字面量或设备字符串范围；P001 的显式 dtype 参数以该值范围定位，`.double()` 以调用范围定位。

## 8 八条规则详细规格

各规则示例中的“命中”仅指该规则；完整扫描可能产生其他规则诊断。示例都是待分析源码，GSU 不运行它们。

### 8.1 T001 transfer-in-loop

**目的**：提示循环中的潜在设备传输。

**触发条件**：调用位于第 6.5 节的重复上下文；接收者确认是 Tensor；调用为 `.cuda()`、`.cpu()` 或可解析 device 重载的 `.to(...)`；转换涉及 CUDA，且不能证明源设备与目标设备相同。源 device 未知、目标 CUDA 时允许 medium；已知 CPU→CUDA 或 CUDA→CPU 时 high。cuda→cuda 不同固定索引可报告；未知目标不报告。

**不触发**：循环外转换；纯 dtype `.to(torch.float32)`；已知 CPU→CPU；同固定 CUDA 索引的调用，即使另外改变 dtype 或设置 copy=True；未知对象同名方法；无法解析目标 device。

**误报边界**：新 batch 的传输可能完全必要；循环不一定执行；non_blocking=True 不消除传输，仍可命中；静态无法确认 overlap、pinned memory 或实际复制耗时。`.to(copy=True)` 可强制复制，但已知同设备的复制不属于本规则；本期不扩展为通用 copy 规则。

```python
import torch
x = torch.ones(4, device="cpu")
for i in range(10):
    y = x.to("cuda")  # T001 high confidence

z = torch.ones(4, device="cuda:0")
for i in range(10):
    z = z.to("cuda:0")  # 不触发 T001
```

**诊断文案**：`Potential device transfer inside a loop.`

**解释**：`This conversion may transfer tensor data during each iteration. Static analysis does not measure its cost.`

**建议**：`Review whether the transfer is necessary for each batch. Move it earlier only if the data and device requirements remain valid.`

**测试点**：CPU→CUDA 正例、CUDA→CPU 正例、固定索引跨卡正例、同设备反例、dtype-only 反例、non_blocking 正例、Tensor 注解 medium、未知对象反例、循环中函数定义反例、while 条件正例、for iterable 反例、分支重赋值后 confidence 降级。

### 8.2 T002 device-ping-pong

**目的**：定位可见的设备往返序列。

**触发条件**：同一直线块或调用链内，同一 provenance 的 Tensor 经过 CPU→CUDA→CPU 或 CUDA→CPU→CUDA；必须明确起始设备，最终返回同一起始设备类，若 CUDA 索引已知则最终索引必须相同；两次转换之间没有中间结果消费或屏障。以返回端调用报告，high/high。

**不触发**：只是单向传输；中间结果用于 print、numpy、计算、存盘或未知调用；不同 Tensor；源设备未知；跨条件分支或跨循环；CUDA:0→CPU→CUDA:1；未知 device 表达式。

**误报边界**：即使没有看到消费，copy、别名、autograd 或同步意图仍可能使序列有意义；不得声称数据绝对无用。检查器接受漏报，不跨未知操作推测冗余。

```python
import torch
x = torch.ones(4, device="cpu")
y = x.cuda().cpu()  # T002，定位最终 .cpu() 调用

z = x.cuda()
consume(z)          # 屏障，中间结果被消费
w = z.cpu()         # 不触发 T002
```

**诊断文案**：`Potential device round trip without an intervening use.`

**解释**：`The visible conversion sequence returns this tensor to its starting device without a tracked intermediate use.`

**建议**：`Review the purpose of both transfers and remove the round trip manually only if its synchronization and data semantics are unnecessary.`

**测试点**：两个方向、链式与局部赋值、简单别名、不同对象、重赋值、未知调用、分支屏障、CPU 中间消费、CUDA 索引变化、起点 Unknown、三次转换的闭合点去重。

### 8.3 S001 item-in-loop

**目的**：提示循环中 Tensor 标量读取可能引入主机等待。

**触发条件**：重复上下文内，确认 Tensor 的无参数 `.item()`；已知 CUDA 时 high confidence，device 未知时 medium；已知 CPU 不报告。Severity medium。

**不触发**：循环外 `.item()`；CPU Tensor；普通 dict/自定义对象方法；无法确认 Tensor 的 model 输出或未注解 loss 变量。

**误报边界**：日志、提前停止和控制流可能需要每步读取；不推断调用频率或标量合法性；`torch.compile` graph break 不属于此规则的承诺。CUDA Tensor 的 `.item()` 可能引入主机等待，参见 PyTorch 官方说明：[CUDA basics](https://github.com/pytorch/pytorch/wiki/CUDA-basics)。

```python
import torch
loss = torch.ones((), device="cuda")
for step in range(10):
    print(loss.item())  # S001 high confidence

cpu_value = torch.ones((), device="cpu")
for step in range(10):
    print(cpu_value.item())  # 不触发
```

**诊断文案**：`Tensor scalar extraction inside a loop may synchronize with the host.`

**解释**：`Reading a CUDA tensor as a Python scalar may require waiting for device work.`

**建议**：`Review whether scalar reads can be less frequent or aggregated without changing control flow or logging requirements.`

**测试点**：CUDA、CPU、Unknown device、Tensor 注解、假同名方法、嵌套 loop、条件日志仍命中、循环外读取、lambda 上下文隔离、非法带参数调用不匹配。

### 8.4 S002 explicit-sync

**目的**：定位显式 CUDA 全设备同步调用。

**触发条件**：规范名为 `torch.cuda.synchronize` 的调用，包括 import alias；不要求位于循环。循环内 severity high，其他位置 medium；confidence high。

**不触发**：被参数或赋值遮蔽的 synchronize；自定义对象 `.synchronize()`；CUDA Event/Stream 的方法，本期不推断其类型；字符串、注释中的名字。

**误报边界**：计时、调试和跨流数据正确性可能依赖同步。不能建议直接删除。此 API 的等待语义及计时用途参考：[torch.cuda.synchronize](https://docs.pytorch.org/docs/main/generated/torch.cuda.synchronize.html)、[CUDA semantics](https://docs.pytorch.org/docs/main/notes/cuda.html)。

```python
import torch
for i in range(10):
    torch.cuda.synchronize()  # S002 high severity

torch.cuda.synchronize()      # S002 medium severity

def f(torch):
    torch.cuda.synchronize()  # 不触发，名字被遮蔽
```

**诊断文案**：`Explicit CUDA synchronization may limit asynchronous execution.`

**解释**：`This call waits for CUDA work on the selected device. Its necessity depends on timing and correctness requirements.`

**建议**：`Review why synchronization is required here before changing it, especially in timing or cross-stream code.`

**测试点**：完整名、模块别名、from alias、device 参数、循环等级、函数局部遮蔽、Event 反例、计时场景仍报告但保留条件化建议。

### 8.5 D001 hardcoded-device

**目的**：提示固定 CUDA 索引，属于配置和可移植性检查点。

**触发条件**：标准 `torch.device("cuda:N")`、`torch.device("cuda", N)`；确认 Tensor 的 `.to("cuda:N")`/`.cuda(N)`；支持工厂的 `device="cuda:N"`。N 为非负整数字面量；允许静态局部常量传递，定位原始字面量，并按该范围去重。severity low、confidence high。

**不触发**：无索引 `"cuda"`；普通字符串 `message="cuda:0"`；动态 local_rank/env 值；固定 CPU；未知对象 `.cuda(0)`。不扫描所有字符串，也不额外支持 set_device API。

**误报边界**：固定索引可能是明确设计，不表示性能问题；CUDA_VISIBLE_DEVICES 会影响可见索引含义；不得据此声称物理 GPU 绑定错误。

```python
import torch
device = torch.device("cuda:0")  # D001
x = torch.zeros(4, device="cuda:1")  # D001
message = "cuda:0"                # 不触发
chosen = torch.device("cuda")      # 不触发
```

**诊断文案**：`CUDA device index is hardcoded.`

**解释**：`A fixed CUDA index may restrict device configuration or portability; it is not inherently a performance defect.`

**建议**：`Review whether this device index should come from the application's existing configuration.`

**测试点**：两种 torch.device 形式、cuda(0)、工厂 keyword、局部常量传播与去重、一般字符串、动态索引、负整数不匹配、CPU、索引多位数、同名遮蔽。

### 8.6 D002 repeated-device-cast

**目的**：提示静态可证的重复设备转换表达式，不宣称重复传输。

**触发条件**：同一直线块或链式表达式中，确认 Tensor 已经经过一次 device 转换，后续再次转换至同一确定设备；无中间消费或屏障；两次有效参数等价，copy 缺省或 false，不改变 dtype/memory_format。比较使用明确 CPU 或固定 CUDA 索引，v0.1 不对 `"cuda"` 当前设备做冗余证明。severity low、confidence high。

**不触发**：不同设备；仅工厂创建后一次 to；第二次 copy=True；dtype 改变；memory_format 显式变化或无法解析；未知参数；跨循环迭代；不同 Tensor；中间未知调用。

**误报边界**：显式重复转换可能作为 API 边界防御性代码。相同 dtype/device 时 Tensor.to 可以返回原对象，不能称为真实 copy。[Tensor.to 官方说明](https://docs.pytorch.org/docs/main/generated/torch.Tensor.to.html)。

```python
import torch
x = torch.ones(4, device="cpu")
y = x.to("cuda:0").to("cuda:0")  # D002，第二个调用
z = x.to("cuda:0").to("cuda:1")  # 不触发 D002
w = x.to("cuda:0").to("cuda:0", copy=True)  # 不触发
```

**诊断文案**：`Repeated conversion to the same known device.`

**解释**：`This conversion appears redundant. It may return the existing tensor rather than copying data.`

**建议**：`Review whether the repeated conversion is needed to express an API boundary or defensive device handling.`

**测试点**：固定 CUDA、CPU 重复、不同设备、copy=True、dtype/memory_format 变化、局部赋值串联、未知调用、同名方法、当前 CUDA 未指定索引不报告、设备常量规范化。

### 8.7 P001 explicit-float64

**目的**：提醒评估显式 FP64 的必要性。

**触发条件**：支持的 torch 工厂显式 `dtype=torch.float64`/`torch.double`；确认 Tensor 的 `.double()` 或 `.to(torch.float64)`/`to(dtype=...)`。支持 import alias 和静态 dtype 变量；不要求 CUDA 或循环。severity medium、confidence high，表示“显式 FP64”证据强。

**不触发**：Python `float`、NumPy dtype、普通字符串、未指定 dtype、仅修改全局默认 dtype、未知接收者 `.double()`、推测出的 FP64。全局默认修改本期不作为 P001 新触发形式。

**误报边界**：科学计算、稳定性或算法可能需要 FP64；不同 GPU 的 FP64 能力不同，CPU 场景也不能推断性能损失。不能自动推荐 FP16/BF16，也不能断言 float32 保持结果。

```python
import torch
x = torch.ones(4, dtype=torch.float64)  # P001
x = x.double()                        # P001
value = float(1)                      # 不触发
z = torch.ones(4)                     # 不触发
```

**诊断文案**：`Explicit float64 tensor precision; review whether it is required.`

**解释**：`Float64 is explicitly requested. Its cost and necessity depend on hardware and numerical requirements.`

**建议**：`Validate numerical accuracy and performance before choosing a lower precision.`

**测试点**：float64/double 常量别名、工厂、double 方法、to dtype 两形式、局部 dtype 别名、Python float、NumPy、未知方法、CPU 仍报告、与 P002 并存。

### 8.8 P002 repeated-dtype-cast

**目的**：提示循环内可能重复发生的 dtype 转换，不要求静态证明跨迭代冗余。

**触发条件**：重复上下文中确认 Tensor 的 `.float()`、`.half()`、`.double()`、`.bfloat16()` 或显式 dtype 的 `.to(...)`；目标 dtype 已解析，源 dtype 不同或 Unknown。源与目标均确定且不同为 high，源未知为 medium；severity medium。

**不触发**：循环外；已知相同 dtype 且 copy=False/缺省；纯 device 转换；未知接收者；动态 dtype；仅 `.int()`/`.long()` 等不在首批支持清单中的方法。copy=True 的同 dtype 操作不属于 dtype 转换，留给未来 copy 规则。

**误报边界**：每批新输入的转换可能合理；dtype 转换可能用于数值安全；float/half 可能在目标 dtype 已匹配时无实际转换，Unknown 情况只能用 may。不得直接建议改变全局默认 dtype。

```python
import torch
x = torch.ones(4, dtype=torch.float64)
for i in range(10):
    y = x.float()  # P002 high confidence

z = torch.ones(4, dtype=torch.float32)
for i in range(10):
    y = z.float()  # 不触发 P002
```

**诊断文案**：`Potential repeated dtype conversion inside a loop.`

**解释**：`This cast may convert tensor values during each iteration. The required input precision is not inferred.`

**建议**：`Review whether the input dtype can be prepared earlier while preserving numerical behavior and per-batch requirements.`

**测试点**：四种方法、to dtype、已知相同 dtype、不同 dtype、Unknown medium、每轮重新赋值、device-only、动态 dtype、未知对象、循环外、copy=True 同 dtype 反例、P001/P002 同时启用。

## 9 CLI 规范

### 9.1 命令语法

```text
gsu --help
gsu --version
gsu check [PATH ...]
    [--select CODE_OR_PREFIX,...]
    [--ignore CODE_OR_PREFIX,...]
    [--exclude PATTERN]...
    [--config FILE]
    [--output-format console|json]
gsu rule RULE_ID
```

无 PATH 时等价于 `gsu check .`。无子命令的 `gsu` 打印帮助并返回 0。`gsu rule` 缺少编号返回 2；`gsu rule T001` 输出规则完整说明，返回 0。v0.1 不支持 rule 的 JSON 模式。

`--help`/`--version` 优先使用参数库的标准行为，写 stdout，返回 0，不发现文件、不读取配置。版本行固定为 `gsu 0.1.0`，构建元数据如有需要进入附加版本段，不能包含环境路径。

### 9.2 参数细则

| 参数 | 行为 |
| --- | --- |
| PATH | 文件或目录；支持多个，去重；以 `-` 开头的路径用 `--` 分隔 |
| --select | 单次，逗号列表；替换配置的 select |
| --ignore | 单次，逗号列表；替换配置的 ignore |
| --exclude | 可重复；出现时整体替换配置 exclude，不影响内建排除 |
| --config | 显式 TOML 文件；必须包含 `[tool.gsu]` |
| --output-format | 默认 console，仅影响 check |

select/ignore token 仅接受大写精确规则编号或 T/S/D/P 类别前缀。未知编号、空 token、小写编号、任意前缀、重复的单次参数返回 GSU-E001 或 GSU-E003。token 重复可去重；类别展开后 ignore 总是从 select 集合中扣除。没有 `--fix`、`--watch`、`--run`、`--exit-zero`、严重度阈值或 stdin `-`。

包含通配符的 PATH 不由 GSU 自行展开；shell 可以先展开。应通过目录参数和 exclude 控制批量扫描。所有参数说明必须出现在 `gsu check --help`。

### 9.3 典型返回行为

```text
有效文件，无诊断                         → 0
有效文件，至少一条未忽略诊断             → 1
有效文件有诊断，另一个文件解析失败       → 2
规则全部忽略，Python 文件仍有语法错误    → 2
目录中没有可扫描文件                     → 0，files_checked=0
显式指定不存在的文件                    → 2
```

即使 select 最终为空，仍执行发现与解析并报告操作错误；这是为了不把解析失败误包装成检查成功。空集合配置合法，但应由文档示例提醒用户它不会产生规则诊断。

## 10 配置与文件发现

### 10.1 最小配置

```toml
[tool.gsu]
select = ["T", "S", "D", "P"]
ignore = ["D001"]
exclude = ["generated", "**/vendor/**"]
```

三个键均为字符串数组。缺省 select 为全部八条；ignore、exclude 缺省为空。空 select 为不启用规则；空 ignore 为不忽略。未知 `[tool.gsu]` 键、错误类型、未知规则或无效 glob 必须报错，不能静默忽略。其他工具的 TOML 表不校验，但 TOML 文件整体必须能解析。

### 10.2 配置定位与优先级

1. `--config FILE` 存在时仅使用该文件；项目根为该文件所在目录。
2. 否则从当前工作目录向父目录查找第一个含 `[tool.gsu]` 的 `pyproject.toml`；项目根为其目录。遇到 `.git` 所在目录时，检查该目录后停止；没有 `.git` 时查到文件系统根。
3. 查找路径上的无效 TOML 报 GSU-E002；合法但不含 `[tool.gsu]` 的文件继续向上。
4. 找不到配置则项目根为当前工作目录，使用默认值。
5. 本期每次命令只有一个配置，扫描时不发现嵌套 pyproject，不级联合并。检查其他目录也不会改变配置查找起点。
6. 每个字段按“CLI 明确提供 > 配置 > 默认值”替换。不同来源字段组合后再计算 selected minus ignored。

例如配置 ignore D001 后，`--select D` 仍忽略 D001。需要清空 ignore 时使用 `--ignore ''`，该完整空字符串特例表示空列表；`--select ''` 同理表示空 select。列表中的空项如 `T,,S` 仍报错。此特例必须在 CLI 测试覆盖。

本期不支持环境变量配置、用户全局配置、专用 gsu.toml、extend、per-file-ignores 或 `# noqa`/`# gsu: ignore`。源码中的这些注释没有抑制作用，避免无文档的隐藏行为。

### 10.3 文件过滤顺序

先校验输入路径，再按下列顺序过滤：

1. 普通文件/目录检查；拒绝设备文件、FIFO、socket；不跟随符号链接。
2. 内建排除目录：`.git`、`.hg`、`.svn`、`.venv`、`venv`、`__pycache__`、`.mypy_cache`、`.pytest_cache`、`.ruff_cache`、`.gsu_cache`、`build`、`dist`、`node_modules`。
3. 隐藏目录默认跳过；根目录自身即使以 `.` 开头仍可扫描其非隐藏子项。
4. 项目根及其下层 `.gitignore`，仅应用到根内路径。
5. 用户 exclude。
6. 扩展名 `.py`，大小及编码约束。

v0.1 为稳定性关闭全局 gitignore、`.git/info/exclude`、`.ignore` 和父项目根外 ignore 继承；不依赖当前是否是真 Git 仓库，根内 `.gitignore` 始终生效。gitignore 的层级、后匹配优先和否定规则按 Git 风格处理，但不能重新包含被内建排除或用户 exclude 的内容；被剪枝父目录下的否定规则不可见。

### 10.4 exclude 的精确语义

以项目根相对路径匹配，统一 `/`，区分大小写。Windows CLI 输入中的反斜杠先转成 `/`。采用 globset 支持的 `*`、`?`、`**` 与字符集合语法；不支持 `!` 否定或 shell brace expansion。含无效模式返回 2。

无 `/` 的非 glob 字面项如 `generated` 匹配任何同名路径组件，并剪枝其子树；带 `/` 的模式按根相对路径匹配；目录自身匹配后全部子树跳过。为匹配任意层级 Python 文件，使用 `**/*.py`。根外路径按相对根含 `../` 的路径匹配用户 exclude，不加载根外 gitignore。

### 10.5 显式文件与特殊情况

- 显式指定文件仍遵循内建、隐藏和 exclude/gitignore 过滤；不能绕过排除。被过滤时 console stderr 给出说明，退出仍按剩余结果决定；JSON 在 skipped 记录原因。
- 显式非 `.py` 文件报 GSU-E004；目录里非 `.py` 不计入候选文件。
- 显式符号链接报 GSU-E004；遍历遇到符号链接仅跳过，不检查其目标。遍历中仅名字以 `.py` 结尾的链接计入候选及 skipped；其他链接不计入文件统计。
- 同一规范化绝对路径只分析一次；不做硬链接 inode 去重。Windows 路径去重依平台路径语义测试。
- 目录无法读取或文件扫描期间消失、无法读取都记操作错误并返回 2。
- 不通过 git 命令发现文件，避免启动子进程。
- `.gitignore` 文件无效或无法读取时记录 GSU-E007，不能悄悄改成全量扫描。

## 11 输出与 JSON Schema

### 11.1 Console

```text
train.py:4:9: T001 Potential device transfer inside a loop.
  4 |     y = x.to("cuda")
    |         ^^^^^^^^^^^^
  Severity: medium  Confidence: high
  Explanation: This conversion may transfer tensor data during each iteration.
  Suggestion: Review whether the transfer is necessary for each batch.

Checked 1 file; found 1 diagnostic; 0 errors.
```

诊断和最终 summary 写 stdout，操作错误与显式跳过说明写 stderr。v0.1 建议无颜色，减少 TTY 差异。源码摘录最多 3 行，每行最多 200 个显示字符，过长截断并显示省略标记，真实 location 不变；多行调用仅展示开始行和必要上下文。

终端中的源码、路径和错误信息必须转义 ESC、控制字符和不安全双向控制字符；JSON 按 JSON 编码保留数据，不混入 ANSI。输出不包含机器绝对 home 路径或环境变量，除非错误无法用根相对路径表达。

稳定排序键为 `(path, start.line, start.column, rule, end.line, end.column)`。文件枚举顺序、HashMap 顺序不能影响结果。不得在 JSON 内加入时间戳、耗时、随机 ID，确保同输入同输出。

### 11.2 JSON 示例

`gsu check --output-format json` 在 stdout 只输出一个完整 JSON 对象并以换行结束。字段为 snake_case；Severity/Confidence 小写。

```json
{
  "schema_version": "1",
  "tool_version": "0.1.0",
  "complete": true,
  "diagnostics": [
    {
      "rule": "T001",
      "severity": "medium",
      "confidence": "high",
      "location": {
        "path": "train.py",
        "start": {"line": 4, "column": 9},
        "end": {"line": 4, "column": 21}
      },
      "message": "Potential device transfer inside a loop.",
      "explanation": "This conversion may transfer tensor data during each iteration. Static analysis does not measure its cost.",
      "suggestion": "Review whether the transfer is necessary for each batch. Move it earlier only if the data and device requirements remain valid.",
      "evidence": [
        {"kind": "loop_context", "detail": "for body"},
        {"kind": "device_conversion", "detail": "cpu to cuda"}
      ]
    }
  ],
  "errors": [],
  "skipped": [],
  "summary": {
    "files_discovered": 1,
    "files_checked": 1,
    "files_failed": 0,
    "files_skipped": 0,
    "diagnostics": 1
  }
}
```

files_discovered 计通过目录过滤后发现的 `.py` 候选和显式 `.py` 输入；显式被排除文件也记录为候选及 skipped。剪枝目录内部文件不计数。扫描中遇到明确的 `.py` 文件后因 hidden、excluded 或 symlink 被跳过，才计入候选与 skipped；对未枚举的目录子树不生成逐文件 skipped。files_checked 为读取和解析成功且完成规则处理的文件数；files_failed 为候选文件读取/解析/分析失败数；发现目录失败不虚构文件数量。正常完整流程满足 discovered = checked + failed + skipped；全局提前失败/资源中止时 summary 只描述已知候选，complete=false。

### 11.3 JSON Schema 草案

以下为 Draft 2020-12，可直接保存为 `schemas/diagnostics-v1.schema.json`。schema_version 描述报告结构版本，tool_version 描述软件版本；两者独立。

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "GSU diagnostic report version 1",
  "type": "object",
  "additionalProperties": false,
  "required": ["schema_version", "tool_version", "complete", "diagnostics", "errors", "skipped", "summary"],
  "properties": {
    "schema_version": {"const": "1"},
    "tool_version": {"type": "string", "minLength": 1},
    "complete": {"type": "boolean"},
    "diagnostics": {
      "type": "array",
      "items": {"$ref": "#/$defs/diagnostic"}
    },
    "errors": {
      "type": "array",
      "items": {"$ref": "#/$defs/error"}
    },
    "skipped": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["path", "reason"],
        "properties": {
          "path": {"type": "string"},
          "reason": {"enum": ["excluded", "symlink", "hidden"]}
        }
      }
    },
    "summary": {
      "type": "object",
      "additionalProperties": false,
      "required": ["files_discovered", "files_checked", "files_failed", "files_skipped", "diagnostics"],
      "properties": {
        "files_discovered": {"type": "integer", "minimum": 0},
        "files_checked": {"type": "integer", "minimum": 0},
        "files_failed": {"type": "integer", "minimum": 0},
        "files_skipped": {"type": "integer", "minimum": 0},
        "diagnostics": {"type": "integer", "minimum": 0}
      }
    }
  },
  "$defs": {
    "position": {
      "type": "object",
      "additionalProperties": false,
      "required": ["line", "column"],
      "properties": {
        "line": {"type": "integer", "minimum": 1},
        "column": {"type": "integer", "minimum": 1}
      }
    },
    "location": {
      "type": "object",
      "additionalProperties": false,
      "required": ["path", "start", "end"],
      "properties": {
        "path": {"type": "string", "minLength": 1},
        "start": {"$ref": "#/$defs/position"},
        "end": {"$ref": "#/$defs/position"}
      }
    },
    "diagnostic": {
      "type": "object",
      "additionalProperties": false,
      "required": ["rule", "severity", "confidence", "location", "message", "explanation", "suggestion", "evidence"],
      "properties": {
        "rule": {"enum": ["T001", "T002", "S001", "S002", "D001", "D002", "P001", "P002"]},
        "severity": {"enum": ["low", "medium", "high"]},
        "confidence": {"enum": ["low", "medium", "high"]},
        "location": {"$ref": "#/$defs/location"},
        "message": {"type": "string", "minLength": 1},
        "explanation": {"type": "string", "minLength": 1},
        "suggestion": {"type": "string", "minLength": 1},
        "evidence": {
          "type": "array",
          "minItems": 1,
          "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["kind", "detail"],
            "properties": {
              "kind": {"type": "string", "minLength": 1},
              "detail": {"type": "string", "minLength": 1}
            }
          }
        }
      }
    },
    "error": {
      "type": "object",
      "additionalProperties": false,
      "required": ["code", "message", "path"],
      "properties": {
        "code": {"type": "string", "pattern": "^GSU-E[0-9]{3}$"},
        "message": {"type": "string", "minLength": 1},
        "path": {"type": ["string", "null"]}
      }
    }
  }
}
```

Schema 之外必须做语义断言：位置 start ≤ end；summary.diagnostics 等于数组长度；errors 非空意味着 complete=false；complete=true 才可能返回 0/1。测试不能只验证 JSON 语法。

### 11.4 错误时 JSON 行为

参数成功解析且选择 JSON 后，配置、文件和解析错误也输出同一 envelope：`complete=false`，errors 非空，保留已完成文件的 diagnostics，退出 2。不要把错误消息混在 stdout JSON 前面。

参数自身无效时允许参数库向 stderr 打印文本并退出 2，不保证 JSON envelope；stdout 为空。输出写入失败时无法保证完整 JSON，返回 2；调用者必须先检查退出状态再消费结果。此例外需在集成文档明确说明。

## 12 错误处理与退出码

| 错误码 | 场景 | 继续策略 |
| --- | --- | --- |
| GSU-E001 | 无效 CLI 参数、未知命令 | 不扫描 |
| GSU-E002 | TOML 解析、键或类型错误、显式配置不可读 | 不扫描 |
| GSU-E003 | 无效规则选择或 `gsu rule` 未知编号 | 不扫描 |
| GSU-E004 | 输入不存在、显式不支持文件、非普通输入 | 其他输入继续 |
| GSU-E005 | 源码非 UTF-8 或无法解码 | 其他文件继续 |
| GSU-E006 | Python 语法错误或 Parser 不支持 | 其他文件继续 |
| GSU-E007 | 目录、文件、ignore 读取失败 | 可独立输入继续 |
| GSU-E008 | 文件或分析资源超限 | 按限制范围停止 |
| GSU-E009 | stdout/stderr 输出失败 | 停止，无法保证报告完整 |
| GSU-E010 | 内部未预期错误 | 停止当前分析，返回 2 |

退出码计算优先级：任何操作错误 → 2；否则未忽略诊断非空 → 1；否则 → 0。Low severity 也返回 1，不能暗中当作纯提示；`--ignore` 过滤后的诊断才计数。

文件语法错误属于无法完成分析，不属于性能规则。禁止将其包装为 T/S/D/P 诊断。错误文案含相对路径和可操作说明，例如 `GSU-E005: train.py is not valid UTF-8; convert the file encoding before checking.`

正常可恢复错误通过 Result 返回，不能 panic。最外层可捕获 unwind 并转 GSU-E010，但不能承诺捕获 OOM、栈溢出或外部强杀；SIGINT/SIGTERM 使用平台约定，可能返回 130 等系统状态，这不是新增的 GSU 业务退出码。常规输入必须避免触发这类崩溃。

## 13 性能与缓存

### 13.1 性能目标

以下是待测发布目标，不是已测结果。固定 Linux x86_64、本地 SSD、4 个以上逻辑 CPU、16 GiB 内存的参考机；记录具体 CPU、OS、Rust、构建 hash、文件数量、字节数和诊断密度。

| 场景 | 发布目标 |
| --- | --- |
| `--version` | 进程启动到退出 p95 ≤ 50 ms |
| 单文件约 1,000 行 | p95 ≤ 150 ms |
| 100 文件合计约 100,000 行、10 MiB | p95 ≤ 1 s |
| 1,000 文件合计约 1,000,000 行、100 MiB | p95 ≤ 10 s |
| 100 MiB 语料峰值 RSS | ≤ 256 MiB，低诊断密度条件 |
| Linux release 二进制 | strip 后目标 ≤ 20 MiB，不含压缩包 |

语料须包含真实风格训练脚本、复杂语法和大量无 PyTorch 文件，不能只重复简单空行。时间包括发现、读取、分析、序列化到本地文件，不包括交互终端绘制和网络盘。分别记录操作系统页缓存冷/热结果；至少 5 次预热、30 次计时，报告 median/p95/RSS。冷缓存测试需独立环境，不在用户系统执行清缓存命令。

共享 CI 不以绝对时延波动直接阻断每次 PR；固定基准机上相对基线回退超过 20% 且复测成立时阻断发布。任何放宽目标必须记录原因，不能在 README 保留旧宣传值。

### 13.2 资源限制

建议初始硬限制：单文件 2 MiB，单文件 token 数 500,000，括号/缩进结构深度 256，候选文件 100,000，累计诊断 100,000。第一周 Parser 验证必须确认限制足以防御深度输入；若词法器自身会递归溢出，在进入依赖前添加迭代式防护。

单文件限制触发 E008 并继续其他文件；全局文件数/诊断数上限触发后停止后续分析，complete=false，不悄悄截断。文件读取使用有限读取而非先相信 metadata，防止扫描中增长。Token/深度检查必须保留字符串和注释语义，不能用简单字符计数把字符串中的括号误判为结构深度。

限制值应写成带文档的常量并测试边界；v0.1 不新增配置旋钮。性能内存目标不等同于恶意输入下的严格 RSS 上限，后者受 parser 分配行为影响，需 fuzz 与资源用例验证。

### 13.3 缓存决策

v0.1 不做持久化缓存，不创建 `.gsu_cache`，不提供 cache 命令。理由：先建立正确性基线；逐文件分析已经限制内存；缓存会引入配置、规则、Parser 与路径失效逻辑，并产生磁盘副作用。

允许单次进程内复用行索引、符号 ID 和已解析常量。不得因文件无 `import torch` 字面量就跳过解析，避免别名、from import、解析错误或 future 兼容问题。

未来只有基准证明重复扫描是主要成本才评估内容哈希缓存；key 至少覆盖源码内容、GSU/Parser/规则版本和有效配置。不以 mtime 单独判断有效性，仍不在本期实现。

## 14 测试与质量门禁

### 14.1 分层测试

| 层级 | 必测内容 | 产物 |
| --- | --- | --- |
| Parser | 目标语法、损坏语法、BOM、CRLF、Unicode、tab | 输入夹具与位置断言 |
| Semantic | import alias、遮蔽、局部绑定、分支、循环、屏障、to 重载 | 事实与事件快照 |
| 单规则 | 第 8 节全部正例、反例和误报边界 | 每条规则至少 12 个独立用例 |
| 规则组合 | 同位置多规则、select/ignore、稳定去重 | 组合快照 |
| CLI | 命令参数、help/version、0/1/2、stdout/stderr | 子进程集成测试 |
| 配置/发现 | 根查找、嵌套配置不生效、ignore、显式文件、多输入 | 临时项目树 |
| JSON | schema、位置和计数不变量、错误 envelope | schema 校验测试 |
| 安全 | 不执行、不写源码、不联网、控制字符、资源超限 | 隔离 smoke 与 fuzz |
| 发布 | 干净机器解压运行、无 Python/CUDA | 平台 smoke 结果 |

每条规则的 fixture 用例元数据应声明预期规则、位置、Severity、Confidence 和不应出现的规则；不要只检查“输出包含 T001”。至少一个跨规则反例测试避免注册表误映射。

### 14.2 关键回归集合

- 普通对象实现 `.item/.cuda/.to/.double`：不被误认为 Tensor。
- 函数形参 `torch`、局部 `torch = ...` 和 module 重赋值：导入解析失效。
- `to(dtype)` 与 `to(device)`、copy=True、memory_format：分支不混淆。
- 当前 CUDA device 与固定索引不强行相等。
- 同一 loop 中重赋值，不沿用错误的上一轮 device/dtype。
- loop 中仅定义函数不触发该函数的循环规则。
- D001 不扫描注释/普通字符串，常量多次使用不重复定位。
- `synchronize` 用于计时仍提示审查，不输出“删除此调用”。
- JSON 模式失败不混入 console 行，部分扫描失败最终返回 2。
- 所有源码检查前后内容哈希相同；无新文件、缓存或 Python `__pycache__`。

### 14.3 误报评估

建立不少于 20 个具有可记录许可证的真实风格源码样本，覆盖训练、推理、科学计算、日志和多设备配置。样本只用于静态测试，不默认下载或执行用户代码。

人工标注“是否准确描述静态模式”与“是否值得用户检查”两列；不得把静态 precision 宣称为真实性能问题 precision。受支持模式的合成标注集召回率目标 100%；真实样本中错误对象识别必须为 0；有效模式诊断比例目标 ≥ 90%，按规则分别汇报样本数与误报原因。样本不足的规则只报告数量，不给稳定百分比结论。

无法达到目标时先缩小规则触发条件并更新文档与测试，不以 confidence=low 掩盖错误识别。已冻结八条规则均需交付，但允许在明确支持子集内保守检测。

### 14.4 Fuzz 与副作用验证

Fuzz 目标包括 Python 解析入口、作用域事实构建、TOML、glob 和 console 转义；固定时间预算和 seed 语料。PR 跑最小 smoke，nightly 跑较长预算；所有 crash 转最小回归 fixture。

不执行测试：被扫描 `.py` 含创建哨兵文件、网络请求、死循环及异常的顶层代码，扫描后无哨兵，检查快速结束。用隔离测试环境验证不产生子进程和网络访问；不需要真的允许恶意片段执行。

### 14.5 完成定义

一个任务完成需同时满足：实现合入、需求对应测试通过、相关帮助/规则文档同步、没有新增范围外依赖、CI 通过。规则完成还需人工审阅误报文案。发布前阻断条件包括任何崩溃、源码改动、稳定位置错误、JSON 不合法或退出码不一致。

## 15 CI CD 与发布

### 15.1 Pull request 流水线

```text
checkout
→ 校验固定工具链与锁文件
→ cargo fmt --check
→ cargo clippy --locked --all-targets -- -D warnings
→ cargo test --locked
→ JSON Schema 与文档示例校验
→ release 构建 smoke
```

Linux 上执行完整 suite；Windows/macOS 执行跨平台路径、配置、输出、退出码与二进制 smoke。支持的最低 Rust 与固定发布 Rust 都须通过编译测试；MSRV 第一周确认后写入 Cargo.toml，不凭猜测填数。

夜间任务增加 fuzz、依赖漏洞/许可证审计和固定语料 benchmark。依赖安全工具（如 cargo-audit、cargo-deny）固定版本；已知高风险漏洞需修复或写出有依据的不可达性豁免，不能静默忽略。

### 15.2 发布产物

初次发行优先官方 Release 下载的单二进制压缩包与 crates.io CLI package。开发用 Python 不进入生产依赖。PyPI、Homebrew、winget、安装脚本不作为 v0.1 阻断项，也不生成 Python import 接口。

建议文件名：

```text
gsu-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
gsu-v0.1.0-aarch64-apple-darwin.tar.gz
gsu-v0.1.0-x86_64-pc-windows-msvc.zip
SHA256SUMS
```

每包包含二进制、LICENSE、第三方许可说明和简短安装说明；不附带用户源码、开发缓存或本机构建路径。Linux 最低 glibc 版本通过固定构建镜像验证后记录，不把任意新系统构建结果称为广泛兼容。

### 15.3 发布流程

1. 合入 release PR：版本、changelog、规则文档、schema、兼容矩阵、基准报告。
2. 完成所有发布门禁，生成候选 artifact；先在无 Python/CUDA 环境验证。
3. 负责人创建受保护 `v0.1.0` tag，触发受控 release workflow。
4. 各目标独立构建，校验包内容、执行 smoke、生成 SHA256 和构建来源记录。
5. 发布到正式渠道；具体发布权限由仓库维护者控制。本计划不代表已经发布。
6. 下载正式包复验 `--version`、正例退出 1、反例退出 0、语法错退出 2。

CI 权限默认只读，只有发布步骤获得最小写权限；第三方 action 锁定 commit。支持时采用签名/构建证明，工具选择在发布 ADR 记录。

### 15.4 版本与回滚

0.1.x 用于 bug 修复；新增诊断行为可能影响 CI，必须在 changelog 说明。规则编号不重新分配，S001/S002 不互换。JSON schema 破坏性变化升级 schema_version，并作为明确兼容变更发布。

若正式包错误，发布修正版并保留已发布版本的可追溯说明；不要用相同版本号覆盖不同二进制。严重缺陷时撤回推荐下载入口并说明影响范围，不能删除校验和掩盖差异。

## 16 兼容性与安全约束

### 16.1 目标兼容性矩阵

| 维度 | v0.1 目标 | 验收方式 |
| --- | --- | --- |
| Linux x86_64 GNU | 正式支持首要平台 | 固定基线镜像和干净环境 smoke |
| macOS Apple Silicon | 正式支持目标 | 原生 runner 构建和运行 |
| Windows x86_64 MSVC | 正式支持目标 | 路径、编码、换行与运行测试 |
| Linux ARM64、musl、Intel macOS | 非本期发布承诺 | 后续独立验证 |
| Python 源语法 | 3.10–3.12 目标 | Parser 语法矩阵，无解释器依赖 |
| PyTorch | 文档列出的稳定 2.x API 子集 | 静态 fixture，不检查安装版本 |
| CUDA | 识别源码中的 CPU/CUDA 设备表达式 | 无 toolkit/driver/GPU 要求 |
| MPS/ROCm/其他设备 | 不做设备性能语义支持 | 不针对其特性推断 |
| 编码 | UTF-8 与 BOM | 中文标识符/路径回归 |

只有 CI 和包验收完成的平台才在 README 标为“支持”；未完成时发布必须延后或显式修改矩阵，不默认勾选。分析 PyTorch 2.x API 子集不等于每个 PyTorch 2.x 版本和所有 overload 均被验证。

### 16.2 安全与副作用约束

- 检查过程只读源码和配置；不写原始文件，不建缓存，不下载依赖。
- 不启动 Python、shell、git、pip 或任何项目进程；不 import 目标模块。
- 不联网、不发遥测、不读取环境凭据、不上传诊断。
- 不加载仓库提供的插件、配置代码或动态规则。
- 文件读取不跟随符号链接；目录遍历不进入特殊文件或无限链接树。
- 源码与路径中的终端控制序列经过转义；文案不能形成终端注入。
- 报告可能包含源码路径与片段，由用户决定是否分享；GSU 不自行发布报告。
- 零副作用指无主动写入或执行；操作系统读取可能更新 atime，不能承诺文件系统元数据绝对不变。
- 重定向 `> report.json` 的文件写入由 shell 按用户命令执行，不是 GSU 修改源码。

## 17 里程碑与执行任务

### 17.1 人力与工期假设

建议基线为 6 周，1 名全职 Rust 开发者，另有负责人每周约 0.5–1 天做规则/发布审阅；总开发工作量约 30 人日。若无人承担审阅与跨平台验证，预留第 7–8 周缓冲。工期是规划估计，第一周完成 Parser spike 后重新确认。

关键路径：Parser 与范围 → 绑定和事实 → 规则 → 端到端输出 → 跨平台发布。最不确定部分是 Python 语义子集和 T002/D002 的保守序列分析。

### 17.2 第 1 周 工程骨架与 Parser 决策

| ID | 工作 | 依赖 | 估计 |
| --- | --- | --- | --- |
| W1-01 | 初始化 Rust binary、许可证、固定工具链与锁文件 | 无 | 0.5 人日 |
| W1-02 | CLI check/rule/help/version 骨架与 0/1/2 收口 | W1-01 | 0.5 |
| W1-03 | Parser spike、目标语法、范围和资源防护验证 | W1-01 | 2 |
| W1-04 | Diagnostic/RunReport、位置转换、规则注册表 | W1-03 | 1 |
| W1-05 | 基础 CI、ADR-001、固定基准语料 | W1-01 | 1 |

验收：无 Python/CUDA 环境可运行 help/version；Parser 能处理确认支持的语法；UTF-8/CRLF/BOM 位置测试通过；损坏输入不崩溃；明确选择 crate/toolchain 版本。若 Parser 失败，先解决此门禁，不开始依赖其 AST 的批量规则开发。

### 17.3 第 2 周 配置 发现和轻量语义

| ID | 工作 | 依赖 | 估计 |
| --- | --- | --- | --- |
| W2-01 | TOML、CLI 覆盖、规则集合与错误校验 | W1-02/04 | 1 |
| W2-02 | 文件发现、exclude/gitignore、路径与编码 | W2-01 | 1 |
| W2-03 | Scope、import alias、遮蔽和局部绑定 | W1-03/04 | 1.5 |
| W2-04 | Tensor 工厂、to 重载、device/dtype 事实与 loop 上下文 | W2-03 | 1.5 |

验收：配置查找与过滤行为都有临时目录集成测试；不因方法同名识别 Tensor；函数局部遮蔽正确；device/dtype 转换事件可供规则直接使用；未知状态明确降级。运行 `check` 对有效/无效文件得到完整结构化结果。

### 17.4 第 3 周 首批单点规则与 Console

| ID | 工作 | 依赖 | 估计 |
| --- | --- | --- | --- |
| W3-01 | S002 与 D001、帮助文档和规则测试 | W2-03/04 | 1 |
| W3-02 | P001 与 S001 | W2-04 | 1.5 |
| W3-03 | Console 定位、转义、稳定排序 | W1-04 | 1 |
| W3-04 | 首轮真实样本静态审阅与反例补充 | W3-01/02 | 1.5 |

验收：四条规则均有至少 12 个独立场景；S001/S002 编号正确；硬编码字符串只在合法上下文触发；console 能显示中文/tab/长行，不泄漏控制序列。不得用真实 GPU 执行作为测试必要条件。

### 17.5 第 4 周 转换规则与序列分析

| ID | 工作 | 依赖 | 估计 |
| --- | --- | --- | --- |
| W4-01 | T001/P002 及循环事实失效 | W2-04 | 1.5 |
| W4-02 | 直线块屏障、provenance、T002 | W2-03/04 | 1.5 |
| W4-03 | D002 参数等价与同设备判断 | W4-02 | 1 |
| W4-04 | 八规则组合回归、更新规则说明 | W4-01/02/03 | 1 |

验收：八条规则全部可单独选择；往返只在受支持的闭合序列触发；同设备转换不声称 copy；未知调用/分支后不沿用冗余证明；每批传输建议保留条件；所有规则位置和等级断言通过。

### 17.6 第 5 周 JSON 性能和系统验证

| ID | 工作 | 依赖 | 估计 |
| --- | --- | --- | --- |
| W5-01 | JSON serializer/schema、部分失败 envelope | W1-04/W4 | 1 |
| W5-02 | 发现计数、退出码、错误/输出集成测试 | W2/W5-01 | 1 |
| W5-03 | 大语料基准与热点优化、资源上限 | W4 | 1 |
| W5-04 | fuzz、零执行/零写入与控制字符回归 | W4 | 1 |
| W5-05 | 三平台构建与路径兼容检查 | W5-02 | 1 |

验收：示例报告和所有快照通过 schema 与语义断言；0/1/2 矩阵完整；资源限制不会静默截断；固定基准达到目标或有经审阅的目标调整；安全 smoke 通过；无 Python/GPU 下全流程可用。

### 17.7 第 6 周 发布候选与验收

| ID | 工作 | 依赖 | 估计 |
| --- | --- | --- | --- |
| W6-01 | 真实样本误报审查与最后修正 | W5 | 1.5 |
| W6-02 | README、安装、配置、规则、兼容说明 | W4/W5 | 1 |
| W6-03 | 构建包、许可证、校验和、发布 workflow | W5-05 | 1 |
| W6-04 | 候选包干净环境验收与来源检查 | W6-03 | 1 |
| W6-05 | 版本冻结、release checklist 和发布记录 | W6-01/02/04 | 0.5 |

验收：八条规则、console/JSON、配置、退出码、跨平台包全部达到本文门禁；无未解决发布阻断缺陷；公开文档不宣传 runtime 或自动修改能力；负责人可依据 checklist 直接作出发布决定。

### 17.8 阶段产物与变更控制

- M1（第 1 周末）：可执行骨架、Parser ADR、语法与位置基线。
- M2（第 2 周末）：确定的配置/发现行为、语义事件接口。
- M3（第 4 周末）：八规则可用、文案及误报边界冻结。
- M4（第 5 周末）：端到端输出、性能/安全/平台报告。
- M5（第 6 周末）：可发布包及完整用户文档。

新增需求必须注明属于 bug、实现澄清还是范围扩大；范围扩大移至 v0.2 backlog。若资源不足，优先延后发布和非承诺分发渠道，不移除已冻结规则或降低零侵入约束。

## 18 风险与缓解

| 风险 | 影响 | 缓解与责任任务 |
| --- | --- | --- |
| Parser 新语法或范围不兼容 | 无法扫描或定位错误 | W1-03 先验证，适配层隔离，明确版本子集 |
| Tensor 来源识别过宽 | 非 PyTorch 对象误报 | W2-03/04 使用来源白名单和 Unknown |
| 语义过保守 | 常见 model/DataLoader 代码漏报 | 文档列出限制，v0.2 增强，不伪装完整推断 |
| 循环事实错误继承 | 错误的高 confidence | W4 循环写集合降级及回归 |
| to 重载与 copy 参数混淆 | 把无操作称为传输 | 独立调用规范化和参数测试 |
| 正常传输/同步被当错误 | 用户失去信任 | 审阅建议，Severity/Confidence 独立 |
| FP64 优化暗示错误 | 用户改坏数值结果 | 条件化建议，明确不评估 accuracy |
| JSON 结构漂移 | CI 集成损坏 | schema 和快照门禁，独立 schema 版本 |
| 文件过滤平台差异 | 漏文件或扫描意外内容 | 三平台项目树测试，禁用环境全局 ignore |
| 特制源码耗尽资源 | 卡死或崩溃 | 大小/token/深度限制与 fuzz |
| 依赖体积和升级 | 发布包膨胀或不兼容 | Cargo.lock、ADR、二进制体积跟踪 |
| 工期超支 | 发布延期 | 第一周复估，优先关键路径，保留 1–2 周缓冲 |

风险关闭依据必须是测试、基准或审阅记录，而不是“已考虑”。每个未关闭高风险问题在发布 checklist 中列出责任人和阻断状态。

## 19 v0.2 扩展边界

v0.2 优先候选为：更好的 Tensor 来源推断、有限同文件函数摘要、更多 Python 语法、明确设计的 per-file/inline ignore、可选择的诊断阈值、SARIF、经测量证明需要的缓存。每项单独提出 ADR、兼容影响和测试计划，不默认启用。

这些扩展也必须继续支持 standalone、无需 import、只读源码、离线分析。不能因为增加语义能力就引入用户代码执行。

runtime profiler、`gsu run`、自动 fix、Web/DB/LLM、JAX/TensorFlow/ROCm 不是 v0.2 自动承诺。若未来重新立项，应作为产品范围决策讨论；不在 v0.1 存放占位模块、数据结构或隐藏命令。

## 20 发布核对清单与参考资料

### 20.1 可直接执行的发布 Checklist

- [ ] Rust 工具链、Parser 版本、许可证、MSRV 已明确记录并锁定。
- [ ] 单一 gsu 可执行程序可在无 Python/PyTorch/CUDA 环境运行。
- [ ] check/rule/help/version 行为与帮助一致，无范围外命令。
- [ ] T001/T002/S001/S002/D001/D002/P001/P002 编号、名称与规格一致。
- [ ] 每条规则正例、反例、误报边界、位置和等级测试通过。
- [ ] 作用域遮蔽、分支、循环、未知调用和 to 重载回归通过。
- [ ] Severity/Confidence 分开输出，未声称测量过性能收益。
- [ ] pyproject 配置覆盖、空列表、未知键、无效规则行为已验证。
- [ ] 文件发现、ignore、显式文件、链接和编码行为已验证。
- [ ] console 正确转义并稳定排序，长行与 Unicode 无位置错误。
- [ ] JSON 示例、正常/失败报告通过 schema 与语义断言。
- [ ] 退出码 0/1/2 与部分扫描失败场景通过。
- [ ] 文件、token、深度、数量限制可控，不静默省略诊断。
- [ ] 不执行、不联网、不修改源码、不产生缓存的测试通过。
- [ ] 性能和体积报告记录参考环境，目标和实际值清楚分开。
- [ ] 真实样本误报审阅完成，未解决项及范围限制写入文档。
- [ ] 各承诺平台包在干净环境 smoke 通过。
- [ ] 许可证、校验和、版本、变更记录及发布来源可追溯。
- [ ] 负责人完成候选包验收，正式发行渠道下载复验已安排。

### 20.2 设计依据

1. 用户提供的 `vibe_design.md`：精度、重复复制、设备统一，简约、小、快。
2. 原对话《优化设计方案可行性》的最终收敛及当前任务的冻结范围：standalone Rust CLI、static diagnostics、八条规则、无自动修改。当前冻结要求优先于原始探索方案。
3. [rustpython-parser 官方 API](https://docs.rs/rustpython-parser/latest/rustpython_parser/)：Parser 选型候选与 AST 接口参考，具体依赖由 W1-03 固定。
4. [PyTorch Tensor.to](https://docs.pytorch.org/docs/main/generated/torch.Tensor.to.html)：重载、copy 参数和相同 device/dtype 的返回行为。
5. [PyTorch CUDA synchronize](https://docs.pytorch.org/docs/main/generated/torch.cuda.synchronize.html)：显式同步 API 语义。
6. [PyTorch CUDA semantics](https://docs.pytorch.org/docs/main/notes/cuda.html)：异步执行及正确计时对同步的需求。
7. [PyTorch CUDA basics](https://github.com/pytorch/pytorch/wiki/CUDA-basics)：Tensor 标量读取与主机同步背景。

外部技术资料核对日期为 2026-09-18。官方 main 页面会变化；实施时在 ADR 中记录被验证的 PyTorch 文档版本或 commit。本文规则触发阈值、保守分析策略、工期和性能目标是 GSU 工程设计，不是外部文档提供的性能保证。

