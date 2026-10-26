# 发布版语法复验

固定使用 roplat 0.5.1 / robot_behavior 0.8.1。源码与本轮成功验证程序逐字相同；完整结果见[发布验证](../published-validation.json)。

```sh
cd evaluations/results/20261026/syntax-check
cargo run --locked
cargo clippy --locked -- -D warnings
```

程序检查独立 `motion?; motion >> control.trigger;` 在正常和失败路径上的执行顺序、count 工厂解构和两个 hard 节律的头段声明。均为有限模拟，无硬件连接。

负例复现：在临时副本中，把相邻的 `motion?;` 与 `motion >> control.trigger;` 合并为 `motion? >> control.trigger;`，保留失败断言再运行。本轮在发布 0.5.1 上观察到失败仍进入 control，程序因此以 101 退出。该负例用于记录发布行为，不是已修复的核心缺陷。
