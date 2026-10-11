# 2026-10-12 技能使用评估（E–G）

三个子 agent 分别只读 `skills/roplat-development`、`skills/robot-behavior`、核心 `docs/system-syntax.md`（G 另读 `docs/execution-protocol.md`）与 robot_behavior 源码，在隔离目录完成 [tasks.md](../../tasks.md) 的 E、F、G。核心为分支 `copilot/dsl-resource-lending` 的工作树（`&mut` 资源借出已实现），robot_behavior 为 0.7.0 工作树；设备全部为仿真臂，没有连接硬件。归档只保留源码与清单，构建目录和日志不入库。

| 任务 | 运行 | Clippy `-D warnings` | 结果 |
|---|---|---|---|
| E 检测工位 | 通过 | 通过 | 同一周期内先 `MotionNode` 后控制会话，两次 `&mut arm` 借出按书写顺序；另写了收集型任务节律 |
| F 传送带跟踪 | 通过 | 通过 | 两个兄弟域 + `Latest` 旁路；首次编译即通过 |
| G 守卫与失败 | 通过 | 通过 | `guard?` 使会话失败，System 返回错误，创建层在失败路径上仍断开臂；选择中止整个任务 |

## 发现与处理

1. **核心缺陷（G 发现，已修复）**：Yield 解构后同一值扇出到三个以上消费者时，图规约切出多个通道，通道回写按位置猜测端点失败，回退成把通道编号当节点编号，导致节点每周期重复执行（复现：`first` 每周期 3 次）。修复：规约时记录每个通道实际切断的原子边，按边精确回写；新增编译器单测与语法课程回归测试 `tests/yield_fanout.rs`。原 robot_apps 只断言最终位置，没有覆盖调用次数。
2. **按段收集结果（E）**：`into_rhythm()` 的 Feed/Output 为 `()`，技能未说明。已在 application-design 3.3.1 补充收集型任务节律写法。
3. **周期节律与旁路参数（F）**：`with_period`、`Latest::new(n)` 中 n 的含义、兄弟域结束条件（生产者先结束导致跟随者挂起）未写；已补充到 application-design 3.4。
4. **调用方生命周期示范（E、F）**：robot_apps 的 main 未对传入的臂调用 Lifecycle；已改为调用方连接/断开，并断言 System 不关闭非自建设备。
5. **借出给节律后无法同时读 `.outcome`（G）**：当前“失败后继续下一段”需独立子域；已写入 syntax.md 并记入核心 TODO 待讨论。尾部 `?` 警告的适用范围（仅针对持有本体的使用者）已澄清。
6. **业务拒绝的错误类型（G）**、**导入路径不一致（E）**：已在 syntax.md 与 robot-behavior 技能中说明。

这是有核心背景的同一模型子 agent 的独立使用验证，不是盲评或跨模型保证。

## 修复后复验

在核心 `5d4393a`（0.4.0，含上述修复）与 robot_behavior `13fa7c4`（0.7.0）上：`scripts/verify_examples.py --drives --offline` 的两个范例运行与 Clippy `-D warnings` 通过；`evaluations/verify.py --offline` 复验 2026-09-20 独立产物通过（其锁文件随核心 0.4.0 刷新，源码未改）。2026-10-11 的 task-d 产物使用已被取代的 `Handoff`/`TaskSequence` 写法，保留为历史记录，不再复验。
