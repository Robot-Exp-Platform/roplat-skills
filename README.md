# roplat-skills

面向 AI 的 roplat 技能集合。每个 `skills/<名称>/` 是独立、可移植的技能包，以 `SKILL.md` 为入口；工具可以按自身机制加载该目录，或由用户直接提供入口路径。仓库不要求 Codex、Claude Code 或某个特定编辑器，不会自行安装到全局目录。

| 技能 | 用途 | 入口 |
|---|---|---|
| roplat-development | 用 Node / Rhythm / System 编写和修改 roplat 应用，重点教授设计哲学到写法：节律分层、设备控制节律、资源借出、旁路、生命周期与执行结果 | [SKILL.md](skills/roplat-development/SKILL.md) |
| robot-behavior | 使用或实现 robot_behavior 驱动：类型化运动/控制能力、控制会话契约、原生异步会话与 roplat 适配；由 drives 中的 robot_behavior-skills 迁入并合并为一个技能 | [SKILL.md](skills/robot-behavior/SKILL.md) |

当前技能对应 roplat `0.4.0` 与 robot_behavior `0.7.0`：DSL 修订后多来源须写 `.input.N`、头段值写 `&value`、设备用 `&mut robot` 借出与归还。各技能的版本关联见 [compatibility.json](compatibility.json)。核心仍是未公开发行的内部版本，性能未达标项和已知限制继续保留。

## 使用与验证

让 AI 阅读上述入口，并提供你的需求、roplat 仓库位置和使用版本。涉及多层节律时，入口会引导读取边界说明与真实宏范例。技能资产不依赖本机用户目录；验证命令：

```sh
python3 skills/roplat-development/scripts/verify_examples.py --roplat /path/to/roplat --drives /path/to/drives --offline
```

脚本在临时目录复制例程，并以本地核心路径构建运行，不修改 roplat 或调用机器人。`robot_apps` 依赖 drives 的 `robot_behavior`，不给 `--drives` 时跳过。最后打印临时证据目录。版本关联见 [compatibility.json](compatibility.json)，例程通过不代表所有硬件或跨语言路径通过。

## 集合扩展原则

未来机器人操作技能适合放在同一集合中，但应建立独立的、用途明确的技能（例如某种设备的连接检查、受控执行或故障诊断）。开发知识与实际设备操作分别触发，避免普通编程任务自动执行机器人动作。每项操作技能应说明适用驱动/设备版本、所需运行状态、可观察结果、终止方式及需要的用户授权；这些由真实能力和任务定义，不预造空技能。

技能之间可以链接相关说明，但每个技能的基本使用流程应自含。设备专用参数、凭据和现场状态不写入本仓库。版本更新后用实际任务和可执行例子复验，而非仅校验文档格式。

## 本轮使用验收

2026-10-12（roplat 0.4.0 / robot_behavior 0.7.0）：三个子 agent 只读两个技能与核心语法文档，独立完成检测工位（同周期两次借出）、传送带跟踪（两设备不同节拍）与守卫失败处理三个程序，运行与 Clippy 均通过；其间发现并修复了一个核心调度缺陷（同一值多层扇出时节点被重复执行），并据反馈补充了技能。见[评估记录](evaluations/results/20261012/EVALUATION.md)。

此前：自带三层例程已通过实际运行和 Clippy。另一位未参与技能编写、但已有核心背景的评估者，未读资产实现与验收规则，独立完成了多层采样、错误隔离、兄弟异步域旁路交换三个程序；[产物与结果](evaluations/results/20260920/independent-use/EVALUATION.md)可审阅。这是独立使用验证，不是完全盲评或跨模型能力保证。

复现独立产物：`python3 evaluations/verify.py --roplat /path/to/roplat --offline`。2026-09-21 在干净的 `main` 提交 `c595393` 上重新运行自带例程与此前独立产物，运行断言和 Clippy `-D warnings` 均通过，见[整合后复验记录](evaluations/results/20260921/main-integration-validation.json)。这次复验没有重新进行独立生成评估，也未额外验证 Replay、外语节点或机器人；这些核心能力的工程证据见 [核心整合验收](https://github.com/Robot-Exp-Platform/roplat/blob/c595393df441056a25b2af8bedfcbd0f598b3fec/docs/candidate/integration-validation.md)。此前在 `e1bab70` 上的[发布复验记录](evaluations/results/20260921/publication-validation.json)保留不变。原始日志与构建产物保存在临时目录，仓库只保留代码、锁文件、参数和结果摘要。
