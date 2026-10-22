---
name: roplat-development
description: 使用 roplat 的 Node、Rhythm 和 System 宏编写、修改或排查机器人程序；尤其适用于多层节律域、端口连接、对象生命周期、停止与错误传播。用于开发代码，不用于自行操作真实机器人。
---

# roplat 开发

目标是写出**结构清楚、像机器人应用而非测试代码**的 roplat 程序：使用当前真实 API、保持静态图语义、可编译验证。先核对 [version.json](references/version.json) 与用户仓库版本；已有项目的依赖和当前源码优先。不要用旧 RFC、历史性能原型或旧生成代码猜测语法。服务范围是应用组织（节点如何拆、节律如何分层、对象在哪创建、设备之间如何通信），不包括节点内部算法实现。

## 设计哲学

- **Rhythm 拥有循环，System 拥有拓扑，Node 只拥有计算。** 应用里不手写驱动节点的循环，也不在 System 外循环调用 System。
- **机器人决定自己的节拍。** 每台设备一个控制节律：周期开始取状态，域内算完直接交付指令。
- **多层节律表达任务结构。** 外层分发任务或目标，内层是设备控制节律；只需“到达目标”时用运动节点代替内层。
- **进程内静态，跨设备靠旁路。** 同周期因果数据走图边；不同设备、不同节拍之间用 `Latest` 等旁路交换最新值，不锁步。
- **创建位置就是生命周期。** 跨周期保留的对象在外层创建；每段、每周期的状态在该域头段创建。
- **设备通过独立通道借用。** 用 `&mut arm >> user.resources.0` 把本次调用的可变借用交给使用者；多资源逐个连接，不把设备塞进业务 Input/Output，也不写归还边。

## 工作流

1. 读 [application-design.md](references/application-design.md)，按其“设计步骤”列出设备与节拍、节律分层、节点职责、创建位置和跨域通信，再写代码。
2. 对照 [robot_apps](assets/robot_apps/src/) 中最接近的场景：两层任务×控制（`sequence_control`）、一层运动节点（`sequence_motion`）、由 System 创建并启停设备（`sequence_owned`）、多设备旁路（`dual_arm`）。驱动能力、控制会话与适配器契约见同仓 [robot-behavior](../robot-behavior/SKILL.md) 技能。
3. 按需查语义：语法 [syntax.md](references/syntax.md)；嵌套节律与 Input/Output/Yield/Feed [rhythm-domains.md](references/rhythm-domains.md)；生命周期、停止与错误 [execution-lifecycle.md](references/execution-lifecycle.md)；旁路与外语节点 [communication-and-languages.md](references/communication-and-languages.md)。
4. 运行与验收：[validation.md](references/validation.md)。

## 必须遵守

**用 `#[roplat::system]` 表达应用的节点连接与节律组织。** 节点作者实现 `Node::process`，节律作者实现 `Rhythm::drive`；应用作者不把 System 改写成手工 `process().await` 链。手写展开只用于用户明确要求的底层诊断或性能对照。遇到不支持的结构，先定位限制并给出最小合法写法，或说明需要讨论的核心能力。

一个实例在一层图中只有一个调度位置，跨作用域也只能在一处调用（只有同一 match 的互斥分支可以共用）；读取内层结果用域的 `.output`，不在尾段读内层节点的 `.output`；图边是依赖，不是语句顺序；互不依赖的分支可能并发。普通 Rust 只放在图的头段和尾段。宏负责借用与拥有状态规划，应用代码不写回收钩子或模拟调度器。资源使用者声明 `Resources = (Arm, Camera)`，接收 `(&mut Arm, &mut Camera)`；无资源为 `()`，详见语法参考。

## 交付前核对

实际运行 Rust 项目，在调用方断言结果、调用次数、跨周期状态和需要的生命周期/错误行为；不在节点里埋断言。只报告真实完成的检查，标明硬件、远程服务或不受支持路径。技能存在不扩大用户授权：不控制设备、不上传数据、不发布代码。
