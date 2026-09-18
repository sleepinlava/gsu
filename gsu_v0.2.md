# GSU v0.2 开发与验收说明

日期：2026-09-18。状态：开发候选；正式发布状态见 `docs/release/status.md`。

## 交付目标

在 v0.1 的八条稳定规则上新增八条 PyTorch 性能检查，默认报告采用 Ruff 风格源码标注。保持 Rust standalone、只读、离线、不执行用户代码。`gsu_v0.1.md` 保留为历史规格，本文件说明新增能力和兼容变化。

## 实现批次

1. 报告与规则管理：默认 full，增加 concise，console 作为 full 别名；color auto/always/never；注册表统一维护类别、preview 状态和短建议。
2. Tensor 识别：四种 like 工厂，view/reshape/flatten/transpose/permute/contiguous，以及 sum/mean。
3. 同步与内存：S003–S005、M001–M003。
4. autograd：A001/A002，组合回归、项目样例、schema、fuzz 和基准验证。

代码均已实现；验证记录单独维护，不能把本文件当作跨平台正式发布证明。

## 命令、默认值与兼容性

```bash
gsu check .
gsu check . --output-format concise
gsu check . --preview
gsu check . --preview --select S,M,A
gsu check . --no-preview
gsu check . --color never
gsu check . --output-format json
gsu rule M003
```

- 原有 T001/T002/S001/S002/D001/D002/P001/P002 默认启用；新增规则全部保持 preview。
- `[tool.gsu] preview = true` 可启用实验规则；CLI `--preview` / `--no-preview` 覆盖配置，二者不能同时使用。
- 未指定 select 时选择当前模式允许的全部规则；显式 select 仍限制检查集合，ignore 最后生效。
- 未启用 preview 时精确选择实验编号返回 GSU-E003；类别选择只包含稳定规则。ignore 可以预先列出实验编号。
- full 显示消息、路径行列、起始行源码与下划线、短建议、severity/confidence 和 preview 标记；跨行范围显示结束位置。长解释仍在 JSON 和 rule 帮助中。
- concise 每条一行，保留最终汇总。所有格式保留退出码 0/1/2，部分扫描失败继续检查其他文件。
- auto 仅在终端且未设置 NO_COLOR 时着色；always 显式覆盖，never 禁用。JSON 永不着色。
- 所有 JSON 报告升级到 schema_version="2"，字段结构与位置约定不变。v1 的规则枚举封闭，消费者必须更新版本判断和 schema；保留 v1 schema 用于历史报告，不提供 v1 输出模式。

## 规则清单与边界

| 编号 | 检查 | 条件 |
| --- | --- | --- |
| S003 | 循环内 tolist | 确认 Tensor；已知 CPU 排除，CUDA high，设备未知 medium；声明来源置信度最高 medium |
| S004 | 循环内 bool/int/float(Tensor) | 内置函数未被遮蔽；支持明确 builtins 导入/别名；设备策略同 S003 |
| S005 | 循环内 nonzero | 仅确认 CUDA 的 Tensor；支持函数和方法形式；未知 as_tuple 不推断 |
| M001 | 循环内 empty_cache | 明确 torch.cuda.empty_cache / torch.cuda.memory.empty_cache，无参数 |
| M002 | 循环内 torch.tensor(Tensor) | data 为确认 Tensor；不暗示 detach().clone() 可以避免复制 |
| M003 | 循环内累积 cat | 循环外 Tensor 累加器，直线循环体直接赋值；列表/元组元素为名称，累加器出现一次；排除其他使用、写入、别名、可见逃逸和捕获；只声称潜在重复复制，confidence=medium |
| A001 | 循环内 retain_graph=True | Tensor.backward 或 autograd.backward/grad；create_graph 缺省或显式 False；动态值和 True 排除 |
| A002 | 开启 anomaly detection | detect_anomaly 或 set_detect_anomaly(True)，函数与上下文形式均支持，每个调用报告一次；check_nan 仅识别缺省或静态布尔值 |

新增规则 severity 均为 medium。具体静态证据保存在 diagnostics.evidence。高 confidence 表示模式证据充分，不表示性能收益已测量。计时、调试、共享显存、独立副本、多次 backward 等合理场景必须在帮助中说明。

## 语义实现约束

- 每个新增 API 用有限参数签名拒绝多余位置参数、未知关键字、位置/关键字冲突、`*args` 和 `**kwargs`。
- like 工厂必须有确认的 Tensor 输入。device/dtype 未指定时继承输入；显式动态覆盖使对应事实未知，不使用全局默认值假设。显式 FP64/固定 CUDA 索引仍由 P001/D001 报告。
- 形状操作只接收已知整型维度及静态整型列表/元组；view 的 dtype 重载、动态未知重载不推断。contiguous 只支持缺省或已知 memory_format。
- 视图与可能复制的 reshape/contiguous 保守共享别名身份，清除连续转换历史；未知调用通过一个视图逃逸时，相关视图事实也失效。
- sum/mean 产生新值，保留 device；未指定 dtype 时设为未知，避免错误推断整数 sum 的提升。
- 后求值的参数可能修改先求值的 Tensor，因此参数求值跨越屏障时丢弃过时 device/dtype/转换历史。
- M003 不进行跨迭代固定点分析；检查体引用和别名计数预先汇总，避免对每个候选重复扫描整个循环体。
- 任意模型输出、DataLoader 批次、跨文件类型、动态模块属性修改及任意堆/全局效果均不承诺支持。

## 验证与后续工作

- 原有 96 个规则场景与 25 项集成测试保持回归。
- 126 个新增规则场景独立指定预期编号、位置、severity/confidence，不从程序输出反向生成预期结果。
- 新增 CLI、preview、颜色/快照、Unicode/tab/跨行、算子事实与副作用测试。
- 原有 20 个项目样例继续保留；新增十个原创 MIT 场景及逐例审阅备注，不视作外部项目准确率研究。
- JSON 正常和失败报告通过 v2 schema；fuzz smoke 包含新规则种子。
- 同机同语料比较 v0.1/v0.2 的耗时与内存；超过 15% 回退时分析原因，不用减少检查覆盖换取速度。

后续顺序：同文件返回摘要与模型/容器来源 → autograd 状态与累积图 → DataLoader 和传输关联 → 布局、分配与精度往返。当前不增加自动修复、inline ignore、SARIF、编辑器协议或通用 Python 类型检查。规则转稳定留待后续版本逐项审阅。
