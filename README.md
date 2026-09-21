# roplat-skills

面向 AI 的 roplat 技能集合。每个 `skills/<名称>/` 是独立、可移植的技能包，以 `SKILL.md` 为入口；工具可以按自身机制加载该目录，或由用户直接提供入口路径。仓库不要求 Codex、Claude Code 或某个特定编辑器，不会自行安装到全局目录。

| 技能 | 用途 | 入口 |
|---|---|---|
| roplat-development | 用 Node / Rhythm / System 编写和修改 roplat 程序，重点教授静态组图、多层节律、生命周期与执行结果 | [SKILL.md](skills/roplat-development/SKILL.md) |

当前例程已在核心开发提交 `e1bab70`（`codex/communication-boundary-cost`）上复验。该分支已实现消息静态契约与 Python 布局缓存/返回检查；受限字段的接收值验证仍延期。Replay 新能力在独立提交 `50e5593`，尚未与通信分支整合；使用前须核对实际版本。版本关联见 [version.json](skills/roplat-development/references/version.json)，本技能集的提交不代表核心已冻结或发布。

## 使用与验证

让 AI 阅读上述入口，并提供你的需求、roplat 仓库位置和使用版本。涉及多层节律时，入口会引导读取边界说明与真实宏范例。技能资产不依赖本机用户目录；验证命令：

```sh
python3 skills/roplat-development/scripts/verify_examples.py --roplat /path/to/roplat --offline
```

脚本在临时目录复制例程，并以本地核心路径构建运行，不修改 roplat 或调用机器人。最后打印临时证据目录。版本关联见 [compatibility.json](compatibility.json)，例程通过不代表所有硬件或跨语言路径通过。

## 集合扩展原则

未来机器人操作技能适合放在同一集合中，但应建立独立的、用途明确的技能（例如某种设备的连接检查、受控执行或故障诊断）。开发知识与实际设备操作分别触发，避免普通编程任务自动执行机器人动作。每项操作技能应说明适用驱动/设备版本、所需运行状态、可观察结果、终止方式及需要的用户授权；这些由真实能力和任务定义，不预造空技能。

技能之间可以链接相关说明，但每个技能的基本使用流程应自含。设备专用参数、凭据和现场状态不写入本仓库。版本更新后用实际任务和可执行例子复验，而非仅校验文档格式。

## 本轮使用验收

自带三层例程已通过实际运行和 Clippy。另一位未参与技能编写、但已有核心背景的评估者，未读资产实现与验收规则，独立完成了多层采样、错误隔离、兄弟异步域旁路交换三个程序；[产物与结果](evaluations/results/20260920/independent-use/EVALUATION.md)可审阅。这是独立使用验证，不是完全盲评或跨模型能力保证。

复现独立产物：`python3 evaluations/verify.py --roplat /path/to/roplat --offline`。2026-09-21 在干净的 `e1bab70` 上重新运行自带例程与此前独立产物，运行断言和 Clippy `-D warnings` 均通过，见[本次复验记录](evaluations/results/20260921/publication-validation.json)。这次复验没有重新进行独立生成评估。原始日志与构建产物保存在临时目录，仓库只保留代码、锁文件、参数和结果摘要。
