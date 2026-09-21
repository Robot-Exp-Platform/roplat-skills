# 验证、迁移与排错

先确定用户roplat版本。当前技能例程已在干净的开发提交 `e1bab70` 上复验；版本文件用 `core_commit` 与 `verified_source_digest` 关联实际来源，历史审计指纹保存在 `historical_validation`。Replay 新能力属于另一个平行分支，见 `related_development`；不要把两个分支的能力合称为某个已经整合的版本。开发提交与技能集发布都不等于核心冻结。用当前 `docs/system-syntax.md`、`docs/execution-protocol.md`、`docs/lifecycle.md` 和 `docs/static-ownership-planning.md` 核对契约；旧RFC及skill不是当前依据。候选的 `stop_check` 实验已撤回；错误提示优于猜测宏会忽略参数。

## 可运行例程

```sh
python3 <skill目录>/scripts/verify_examples.py --roplat <核心仓库> --offline
```

脚本复制assets及固定Cargo.lock到新临时目录，写入本地路径依赖并执行cargo run/clippy --locked，不在原仓生成target，不使用机器人/网络，也不安装技能。无离线依赖缓存时可去掉--offline，先按用户环境判断是否允许下载。

[例程源码](../assets/control_layers/src/main.rs)是真实System宏程序。3层有限节律输出、复用对象状态、周期局部生命周期和失败后的关闭顺序均由断言验证。不能把这些特定负载的成功推广为任意DSL语法都支持。

## 常见误用

| 现象 | 首先检查 |
|---|---|
| node只有一个名字却期望调用多次 | 同层一个调度位置；重复由Rhythm产生，或定义不同实例 |
| child后面的节点不运行 | child当前drive是否真的结束，是否无限Rhythm；Output不能早于drive返回 |
| Feed类型不匹配 | 周期尾表达式是否误加分号；child Output和child每轮Feed是否混淆 |
| 外部节点没初始化 | System参数只使用，创建层必须管理hooks |
| 每轮状态消失 | 是否把状态对象创建在周期内 |
| 普通Result导致输入类型不对 | 业务Result默认是数据；明确需要提升时用.output? |
| 停止后再次drive直接Stopped | ExecutionContext粘性；新独立执行使用新context |
| 借用加入spawn后报错 | 跨任务需要Send/'static和拥有对象，不强行unsafe延长引用 |
| 节点开始被当成普通Rust运算 | 图被Rust语句分隔或放在普通if/loop里；回到连续DSL |
| 裸数值被当成Node | 用Source节点或当前Yield/match绑定表达输入 |

修改后运行相关包测试和Clippy，按影响范围选择完整验证。核心/驱动大的契约变更先讨论目的、利弊、兼容性和保留项；技能本身不授权这样的重构。对性能保留版本、机器、时钟分辨率、负载、采样和失败项，不能从展开代码短或Future小直接宣称更快。长期10%回退目标仍存在，本轮允许披露未达标项。消息边界已增加静态契约、padding 初始化编码和 Python 布局/类型检查，但 bool 等受限字段的接收值验证仍按作者决定延期；不能据此宣称可安全接受任意外部字节。Replay 新能力已在独立分支实现；应核对是否已包含相应提交，再使用其 FeedSource 与类型化比较器。有限测试通过不代表所有路径安全、完整或核心版本已成立。

例程独立锁文件本次选择 Tokio 1.52.1，而当前核心工作区锁文件为 1.48.0；它是独立用户工程的兼容验证，不是同锁文件的核心回归测量。核心依赖变化使 `--locked` 失败时，应审阅并重新验证锁文件后更新技能资产，不要在交付脚本中静默解除锁定。
