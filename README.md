# roplat-skills

面向 AI 的 roplat 技能集合。每个 `skills/<名称>/` 是独立、可移植的技能包，以 `SKILL.md` 为入口；工具可以按自身机制加载该目录，或由用户直接提供入口路径。仓库不要求 Codex、Claude Code 或某个特定编辑器，不会自行安装到全局目录。

| 技能 | 用途 | 入口 |
|---|---|---|
| roplat-development | 用 Node / Rhythm / System 编写和修改 roplat 应用，重点教授设计哲学到写法：节律分层、设备控制节律、资源借出、旁路、生命周期与执行结果 | [SKILL.md](skills/roplat-development/SKILL.md) |
| robot-behavior | 使用或实现 robot_behavior 驱动：类型化运动/控制能力、控制会话契约、原生异步会话与 roplat 适配 | [SKILL.md](skills/robot-behavior/SKILL.md) |

当前技能对应 crates.io 上的 roplat `0.5.0` 与 robot_behavior `0.8.0`（robot_behavior 需要 nightly 工具链），见 [compatibility.json](compatibility.json)。两者仍在快速迭代，API 不稳定。技能与例程自成一体，只依赖这两个已发布的包，不需要访问它们的源码仓库。

## 使用与验证

让 AI 阅读上述入口，并提供你的需求、项目位置和使用的版本。涉及多层节律时，入口会引导读取边界说明与可运行范例。验证技能自带的例程（需要 Python 3.11 及以上与 Rust nightly）：

```sh
python3 skills/roplat-development/scripts/verify_examples.py --offline
```

每个例程是依赖 crates.io 包、自带锁文件的独立 Cargo 项目；脚本把它们复制到临时目录，执行 `cargo run --locked` 与严格 Clippy，不连接机器人，最后打印证据目录。没有本地依赖缓存时去掉 `--offline`。例程通过不代表所有硬件或跨语言路径通过。

## 集合扩展原则

未来机器人操作技能适合放在同一集合中，但应建立独立的、用途明确的技能（例如某种设备的连接检查、受控执行或故障诊断）。开发知识与实际设备操作分别触发，避免普通编程任务自动执行机器人动作。每项操作技能应说明适用驱动/设备版本、所需运行状态、可观察结果、终止方式及需要的用户授权；这些由真实能力和任务定义，不预造空技能。

技能之间可以链接相关说明，但每个技能的基本使用流程应自含。设备专用参数、凭据和现场状态不写入本仓库。版本更新后用实际任务和可执行例子复验，而非仅校验文档格式。

## 使用验收记录

2026-10-22（续）：例程改为直接依赖 crates.io 上的 roplat 0.5.0 与 robot_behavior 0.8.0，验证脚本不再需要任何源码仓库；技能文字去掉对核心仓库文档与测试的引用，补齐内置节律、事件与信号、图内 `if` 与守卫、闭包节点的说明。robot_apps 新增两个场景：`plan_hard`（规划放进 hard 节律，控制环最长周期间隔断言；去掉 `.hard()` 后断言失败）与 `task_events`（操作员线程经事件通道发指令，守卫、图内 `if`、`retry` 与信号）。两个例程在 `--locked --offline` 下运行与严格 Clippy 通过，robot_apps 连续运行 10 次稳定，见[复验记录](evaluations/results/20261022/self-contained-validation.json)。

2026-10-22（roplat 0.5.0 发布提交 `8ca570d` / robot_behavior 0.8.0 发布来源 `8694e5b`）：在发布来源上重新运行三层节律与四种模拟机械臂应用，运行断言与严格 Clippy 通过，例程与锁文件未改动，见[发布复验记录](evaluations/results/20261022/v0.5.0-release-validation.json)。技能中 hard 节律、`move` 域、EventTrigger 发送接口与 `create_signal` 的说明已对照核心源码核对，例程本身不使用它们；本轮未重新进行独立使用评估。

2026-10-13（roplat 0.5.0 / robot_behavior 0.8.0）：两个技能及模板完成资源通道迁移，已在提交 `d6484b1` / `1f6bf80` 上重新运行三层节律与四种模拟机械臂应用，严格 Clippy 和技能格式验证通过，见[最终验证记录](evaluations/results/20261013/resource-borrow-final-validation.json)。[开发工作树记录](evaluations/results/20261013/resource-borrow-validation.json)保留原来源。

另一位具有核心实现背景的评估者使用技能编写了双资源失败后复用应用，发现并促成生命周期返回说明与错误类型导入提示修正；这不是完全盲评。其[评估报告](evaluations/results/20261013/skills-forward-validation.json)与[原样保留的源码及复现说明](evaluations/results/20261013/forward-use/README.md)可审阅。历史评估不替代本轮验证。

2026-10-12（roplat 0.4.0 / robot_behavior 0.7.0）：三个子 agent 只读两个技能与核心语法文档，独立完成检测工位（同周期两次借出）、传送带跟踪（两设备不同节拍）与守卫失败处理三个程序，运行与 Clippy 均通过；其间发现并修复了一个核心调度缺陷（同一值多层扇出时节点被重复执行），并据反馈补充了技能。见[评估记录](evaluations/results/20261012/EVALUATION.md)。

此前：自带三层例程已通过实际运行和 Clippy。另一位未参与技能编写、但已有核心背景的评估者，未读资产实现与验收规则，独立完成了多层采样、错误隔离、兄弟异步域旁路交换三个程序；[产物与结果](evaluations/results/20260920/independent-use/EVALUATION.md)可审阅。这是独立使用验证，不是完全盲评或跨模型能力保证。

历史独立产物依赖当时未发布的核心版本，只作记录，不能用公开版本复现，也不作为 0.5 验收。2026-09-21 在当时的核心提交 `c595393` 上重新运行自带例程与此前独立产物，运行断言和 Clippy `-D warnings` 均通过，见[整合后复验记录](evaluations/results/20260921/main-integration-validation.json)；这次复验没有重新进行独立生成评估，也未额外验证 Replay、外语节点或机器人。此前在 `e1bab70` 上的[发布复验记录](evaluations/results/20260921/publication-validation.json)保留不变。原始日志与构建产物保存在临时目录，仓库只保留代码、锁文件、参数和结果摘要。
