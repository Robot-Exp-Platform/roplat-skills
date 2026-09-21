# 通信选择与多语言边界

## 图边与旁路

需要同次计算的因果输入输出时写System边：`source >> controller >> sink`。宏静态组织调用和值移动，图边并不默认变成三缓冲或环形队列。

独立控制域交换状态时使用旁路：

| 需求 | 当前类型 | 需要承担的语义 |
|---|---|---|
| 读取最新状态/指令，允许跳过中间版本 | `Latest<T>` / Publisher / Subscriber | 每读者三槽；不同读者采样时刻不同，不保证同时全局快照 |
| 保留每个成功入队事件的顺序 | `SpscQueue<T>` | 单生产者单消费者；满时返回原值，应用定义背压/重试 |
| 大尺寸不可变CPU载荷 | `FrameBuffer` → `seal()` → `SharedFrame` | Rust分配独占写，seal消耗写权限；clone共享owner不复制字节 |
| 外语不透明对象 | OpaqueData / SendOpaque / SharedOpaque | 跨线程移动/共享及析构线程需要显式unsafe契约，Arc不自动证明安全 |

```rust
let channel = roplat::comm::Latest::<u64>::new(1);
let mut publisher = channel.publisher();
let mut subscriber = channel.subscriber(0);
publisher.publish(7);
assert_eq!(subscriber.latest(), Some(&7));
```

工厂每个端点仅提取一次；取出后端点持有内核生存期。Latest刷新返回的借用不能跨下一次刷新，需保留快照时显式snapshot/clone。Python小消息读取返回副本；共享帧只读view需要匹配的可选buffer扩展。

这些类型用于被组合的异步代码，不要求每个端点独占OS线程。IPC是另一条字节传输/发现/握手路径，不据进程内Latest测试推断IPC的性能和类型安全。

## 本机消息边界（通信分支 e1bab70）

typed IPC 的自定义消息要求 `Copy + NativeMessage`；`#[roplat_msg]` 自动生成契约，手写本机消息可用 `#[repr(C)]` 与 `#[derive(roplat::NativeMessage)]`。引用、指针、String、Vec 和拥有 Drop 资源的消息不能自动通过该静态契约。普通 Rust 图边及进程内泛型旁路不因此要求 NativeMessage。

含 padding 的消息逐字段编码并将间隙置零；无 padding 的消息可以借用原生字节。`publish_owned(Vec<u8>)` 允许后端接管编码缓冲；第三方后端的默认回退仍可能复制。该格式使用本机布局与端序，不是跨架构 wire 格式。

**NativeMessage 不等于 Pod / AnyBitPattern。** bool 等受限字段的接收值验证按作者决定延期，兼容入口仍信任字段字节有效；非法位模式可能导致 Rust 未定义行为。已有长度检查保留，不能把编译期布局能力理解为对任意外部字节的安全保证。迁移与限制以对应核心版本的 `docs/communication-boundary.md` 为准。

## C++ / Python

应用仍是Rust项目：使用roplat宏和build.rs生成可修改的外语节点文件，C++/Python实现节点；Rhythm/System组织留在Rust。生成器所有的文件会重建，用户实现文件仅缺失时生成，避免修改会被覆盖的基类。现有多语言示例和roplat_build的生成工具是核对入口。

在通信分支，Python 透明 Node 在 `on_init` 中验证并缓存 ctypes 布局，返回时只接受预期准确类型或显式 bytes，并检查长度；暂不检查 bool 等字段位模式。手写透明 `TypeBinding` 需要提供 native hooks。返回路径已删除中间 Rust Vec，仍不等于端到端零复制。

缓存跟随对象的一次启用。节点位于周期内部时，每周期都会支付初始化成本；不能为性能悄悄改变用户需要的生命周期。桥接失败仍走现有 panic 路径，不属于合作式 Execution 失败与资源归还保证。Python opaque Node 的历史 thread-local 单槽交接不支持任意多在途、同类型分支与线程迁移。这些边界不同于已独立验证的 Python 旁路 API。

## Replay：先核对分支

例程复验采用的通信分支尚不包含 Replay 新能力。独立开发提交 `50e5593`（`codex/replay-capabilities`）提供 `ReplayFeedSource<D>` 与类型化 `FeedComparator<D>` / `FloatTolerance`。使用前检查目标仓库是否包含这些 API，并阅读该版本的 `docs/replay.md` 与真实 `replay_nested` System 例程。

在 Replay 分支，旧 `OutputOnly` 和 `FeedCheck::Tolerance` 配置在 drive 开始前明确报迁移错误；应分别迁移到 FeedSource 和 `.with_comparator(...)`。FeedSource 把录制的周期 Feed 作为新消费者域的 Yield，不运行原录制域，也不重建原 drive 的完整 Output。类型化比较会解码成真实 D 再比较，不能再把任意序列化字节当作 f64。

录制仍只覆盖成功周期的 Yield/Feed，不记录旁路读取顺序、对象状态、失败/停止决定和外部副作用。消费者仍会执行真实代码；不承诺自动确定性回放或副作用隔离。此前审计快照上的验证证据保留为历史记录，本次技能提交不表示核心已经整合或成立稳定版本。
