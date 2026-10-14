# 独立资源通道使用评估产物

这是 2026-10-13 独立评估者编写的有限 mock 程序；原始评估结果及开发工作树来源保留于 [报告](../skills-forward-validation.json)。源码与锁文件原样保存，仅两个 manifest 转为路径模板，不包含构建产物或日志。

复现时将此目录复制到临时位置，把 `Cargo.toml.in` 和 `probes/Cargo.toml.in` 中的 `@ROPLAT_CRATE_PATH@`、`@ROBOT_BEHAVIOR_PATH@` 分别替换为 JSON 引号包裹的本地 crate 路径，保存为同目录的 `Cargo.toml`。核心与 robot_behavior 的已验证版本、提交及最终复验见 [最终验证记录](../resource-borrow-final-validation.json)。

在临时主项目运行：

```sh
cargo test --locked --offline --all-targets
cargo run --locked --offline
cargo clippy --locked --offline --all-targets -- -D warnings
```

在临时 `probes` 目录分别运行 `cargo check --locked --offline --bin external` 与 `cargo check --locked --offline --bin owned`。前者应通过；后者有意返回编译错误 E0382，证明本层创建且管理 Lifecycle 的对象不能在关闭前移出。不要把这两个 probe 一起作为必须全通过的 `--all-targets` 测试。

只运行模拟设备，无真实硬件或 GUI。编译产物和原始日志留在临时目录。
