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

工厂每个端点仅提取一次；取出后端点持有内核生存期。`Publisher` 唯一且不可复制：不同节点要发布不同含义的信息时，各开一个通道。Latest刷新返回的借用不能跨下一次刷新，需保留快照时显式snapshot/clone。Python小消息读取返回副本；共享帧只读view需要匹配的可选buffer扩展。

这些类型用于被组合的异步代码，不要求每个端点独占OS线程。IPC是另一条字节传输/发现/握手路径，不据进程内Latest测试推断IPC的性能和类型安全。

## 本机消息边界

typed IPC 的自定义消息要求 `Copy + NativeMessage`；`#[roplat_msg]` 自动生成契约，手写本机消息可用 `#[repr(C)]` 与 `#[derive(roplat::NativeMessage)]`。引用、指针、String、Vec 和拥有 Drop 资源的消息不能自动通过该静态契约。普通 Rust 图边及进程内泛型旁路不因此要求 NativeMessage。

含 padding 的消息逐字段编码并将间隙置零；无 padding 的消息可以借用原生字节。`publish_owned(Vec<u8>)` 允许后端接管编码缓冲；第三方后端的默认回退仍可能复制。该格式使用本机布局与端序，不是跨架构 wire 格式。

**NativeMessage 不等于 Pod / AnyBitPattern。** 0.5 不校验 bool 等受限字段的接收值，兼容入口信任字段字节有效；非法位模式可能导致 Rust 未定义行为。长度检查仍然保留，但不能把编译期布局能力理解为对任意外部字节的安全保证。具体契约以 `roplat::comm` 的 crate 文档为准。

## C++ / Python

应用仍是Rust项目：使用roplat宏和build.rs生成可修改的外语节点文件，C++/Python实现节点；Rhythm/System组织留在Rust。生成器所有的文件会重建，用户实现文件仅缺失时生成，避免修改会被覆盖的基类。构建与绑定以 crates.io 上 `roplat_build`、`roplat_cpp`、`roplat_py` 0.5.1 的 crate 文档为准。

当前 Python 透明 Node 在 `on_init` 中验证并缓存 ctypes 布局，返回时只接受预期准确类型或显式 bytes，并检查长度；暂不检查 bool 等字段位模式。手写透明 `TypeBinding` 需要提供 native hooks。返回路径已删除中间 Rust Vec，仍不等于端到端零复制。

缓存跟随对象的一次启用。节点位于周期内部时，每周期都会支付初始化成本；不能为性能悄悄改变用户需要的生命周期。桥接失败仍走现有 panic 路径，不属于合作式 Execution 失败与资源归还保证。Python opaque Node 的历史 thread-local 单槽交接不支持任意多在途、同类型分支与线程迁移。这些边界不同于已独立验证的 Python 旁路 API。

## Replay：录制 Feed 源与类型化比较

roplat 0.5.1 提供 `ReplayFeedSource<D>`、类型化 `FeedComparator<D>` 与 `FloatTolerance`（`roplat::rhythm::replay`）。使用其他版本时先核对公开 API；具体签名查 crate 文档。

需要重放录制的周期输出时使用 `ReplayFeedSource<D>`：它将录制的 Feed 作为新消费者域的 Yield，不运行原录制域，也不重建原 drive 的完整 Output。其 Feed 为 `()`，整次 Output 为完成帧数报告；父域必须等当前 drive 完成才读取报告。需要对重新执行得到的 Feed 作数值或业务比较时，使用 `ReplayRhythm::with_comparator(...)`；它按真实 D 解码并比较，不能把任意序列化字节当作 f64。`ReplayMode::OutputOnly` 和 `FeedCheck::Tolerance` 不是受支持的执行入口，在 drive 开始前报错，不应作为推荐写法。

Replay 的 serde/bincode 字段表示与 native IPC 的本机内存表示不同，即使二者使用同一个消息类型，也不能直接交换这两种字节缓冲。技能自带例程没有执行 Replay，例程通过不算作 Replay 验证。

录制仍只覆盖成功周期的 Yield/Feed，不记录旁路读取顺序、对象状态、失败/停止决定和外部副作用。消费者仍会执行真实代码；不承诺自动确定性回放或副作用隔离。roplat 仍在快速迭代，API 不稳定。
