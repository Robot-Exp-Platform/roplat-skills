# 通信选择与多语言边界

先区分来源：发布技能位于 `main`（crates.io `roplat 0.5.1`）；本 `next` 的所有活动例程使用源码；下文标为“当前主线”的接口来自核心提交 `78fd47ee0a90194feb88f706ca90d6d584331de8`，尚未包含在该发布包中。当前主线的 Cargo 版本号也仍是 `0.5.1`，所以必须同时核对依赖来源和提交，不能只看版本字符串。已有项目按其 `Cargo.lock` 与实际源码选择接口。

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

### 当前主线：消息库拆分

`roplat_native` 承载无运行时依赖的 `NativeMessage`、`TypeBinding`、`#[roplat_msg]` 与 `#[derive(NativeMessage)]`。应用仍可使用原来的 `roplat::native_message`、`roplat::type_binding`、`roplat::NativeMessage` 与 `roplat::roplat_msg`，这些路径是重新导出，不应机械替换所有已有导入。只定义消息而不依赖运行时的库可依赖 `roplat_native`；由于消息宏展开仍引用 `roplat::…`，该库根部需要 `extern crate roplat_native as roplat;`。

公共消息来自 `roplat_msgs`，例如 `Pose`、`JointSample<N>`、`JointState<N>`、`Header` 与 `Stamped<T>`；它也不依赖运行时。使用 SI 单位与弧度，四元数顺序为 `x, y, z, w`；可选字段通过 `valid` 位掩码与 `Option` 访问器表示。按需开启 `nalgebra`（转换与位姿运算）或 `serde`（序列化）。时间来源用 `roplat_msgs::Clock`：`MonotonicClock`（进程内共享起点）、`RealtimeClock`、由仿真器或回放推进的 `ManualClock`；`Stamp` 只在同一时钟内比较，不要在节点或驱动里直接用 `Instant::now()` 打时间戳。核心 `SysTimer` 尚未接入 `Clock`，仿真锁步目前需要节律自己调用 `sleep_until`。具体消息可生成 C/Python 绑定；带常量泛型长度的消息只有 `NativeMessage`，不能据此推断能直接生成外语头文件。行为库的 `roplat_behavior` 及算法包迁移见同仓 [robot-behavior](../../robot-behavior/SKILL.md)。这些新包须使用相互匹配的源码来源，不能给发布例程添加不存在的 registry 版本。

`roplat_build` 会为依赖图中的消息生成头文件/存根；嵌套消息的头文件会 `#include` 其依赖头，Python 存根按顶层模块 `from vector3 import Vector3` 导入依赖（核心 `102fb36` 起；更早的源码仍需按依赖顺序手动包含）。不要把“Rust 消息可编译”当作 C++/Python 绑定已经验证。

### 当前主线：额外的通信场景

| 场景 | 入口 | 应用需要处理的边界 |
|---|---|---|
| 多个生产者汇入一个消费者 | `roplat::comm::bounded_mpsc` | 同时设置条数和字节预算；选择 Error / DropNewest / Wait，错误保留未准入值 |
| 监看订阅者运行中加入退出 | `roplat::comm::broadcast` | 有界历史、独立游标；慢读者处理 `Lagged(n)`，不等于 Latest |
| 跨进程传输变长字节 | `roplat::comm::ipc::variable` | 显式 schema、session/stream、序号和尺寸限额；载荷编解码由应用负责 |

现有 `TcpOptions` 结构体字面量需补齐新增字段，或在明确默认值适合应用时使用 `..Default::default()`；默认满队列策略已改为 Error。类型化同机 IPC 默认尝试共享内存并在失败时回退 TCP，可用 `ROPLAT_IPC_SHM=off` 关闭；变长 envelope 使用 TCP。最新值端点会替换发送端尚未发出的旧帧，但成功发布不等于远端已接收/处理。自定义 `IpcTransport` 的 `publish_latest` / `publish_latest_owned` 有默认回退实现，需另行核对其保留策略。

## C++ / Python

应用仍是Rust项目：使用roplat宏和build.rs生成可修改的外语节点文件，C++/Python实现节点；Rhythm/System组织留在Rust。生成器所有的文件会重建，用户实现文件仅缺失时生成，避免修改会被覆盖的基类。发布路径使用 `roplat_build`、`roplat_cpp`、`roplat_py` 0.5.1；源码路径使用同一核心提交中的配套包。宏与运行时通过精确依赖匹配，局部覆盖一个包不代表整个依赖图已切到当前源码。

发布版 Python 透明 Node 在每次 `on_init` 中验证并缓存 ctypes 布局。当前主线另外按“ctypes 类＋原生布局”在进程内缓存已通过的检查并保留类对象；节点仍执行自己的初始化，重载得到的新类会重新检查，不能据此承诺模块热更新或子解释器隔离。透明消息的输入与输出改走缓冲区协议，每个方向一次拷贝；输入实例由 `tp_new` 创建，不运行 `__init__`。生成的 `on_init` 仅在路径尚不存在时加入 `sys.path`，避免周期内创建节点时重复插入。

两种来源的返回路径都只接受预期准确的 ctypes 类型或显式 bytes，并检查长度；暂不检查 bool 等字段位模式。手写透明 `TypeBinding` 需要提供 native hooks。不能为性能改变用户需要的生命周期，也不能宣称端到端零复制。桥接失败仍走现有 panic 路径，不属于合作式 Execution 失败与资源归还保证。Python opaque Node 的历史 thread-local 单槽交接不支持任意多在途、同类型分支与线程迁移。这些边界不同于 Python 旁路 API。

## Replay：录制 Feed 源与类型化比较

发布版 `roplat 0.5.1` 提供 `ReplayFeedSource<D>`、类型化 `FeedComparator<D>` 与 `FloatTolerance`（`roplat::rhythm::replay`）。以下 Feed 源语义仍适用；当前主线的录制构造、schema 和文件格式另外有破坏性变更，见下一节。

需要重放录制的周期输出时使用 `ReplayFeedSource<D>`：它将录制的 Feed 作为新消费者域的 Yield，不运行原录制域，也不重建原 drive 的完整 Output。其 Feed 为 `()`，整次 Output 为完成帧数报告；父域必须等当前 drive 完成才读取报告。需要对重新执行得到的 Feed 作数值或业务比较时，使用 `ReplayRhythm::with_comparator(...)`；它按真实 D 解码并比较，不能把任意序列化字节当作 f64。`ReplayMode::OutputOnly` 和 `FeedCheck::Tolerance` 不是受支持的执行入口，在 drive 开始前报错，不应作为推荐写法。

Replay 的 serde/bincode 字段表示与 native IPC 的本机内存表示不同，即使二者使用同一个消息类型，也不能直接交换这两种字节缓冲。技能自带例程没有执行 Replay，例程通过不算作 Replay 验证。

普通 `RecordingRhythm` 只覆盖成功周期的 Yield/Feed，不自动记录旁路读取顺序、对象状态、失败/停止决定和外部副作用。消费者仍会执行真实代码；不承诺自动确定性回放或副作用隔离。

### 当前主线：显式录制会话与 RLOG v4

- Yield/Feed 需要实现 `RecordData`，不能只满足旧的 Serde/Clone 约束。结构体可用 `#[derive(Clone, serde::Serialize, serde::Deserialize, roplat::RecordData)]` 和 `#[record(version = 1)]`；schema、不可变快照语义与保守的保留字节核算属于契约，不能把 `Arc<Mutex<_>>` 的浅克隆声称为不可变快照。
- 用 `RhythmMeta::of::<Y, D>(id, name)` 登记类型。`SessionRecorder::wrap`、`maybe_wrap`、`RecordingRhythm::new` 和 SessionPlayer 的类型入口返回 `Result`，先处理 schema/配置错误再运行图；不要沿用发布版直接返回包装器的写法。
- 录制使用有界后台 worker，默认队列满返回错误；允许在启动前明确选择 DropNewest 或 Wait。后台失败传回所属节律；等待持有 N 的周期完整返回，继续保持既有 Execution 与资源归还语义。
- 图结束后显式收集图结果和 `finish_async().await` 结果；同步外部线程可用 `finish()`。Drop 只能请求结束，不能报告写入失败或替代完成协议。图失败也要执行 finish。
- RLOG v4 带 schema、CRC 和完成尾部，不读取或转换旧 JSON-lines/v3 文件。严格校验和显式 RecoverPrefix 是不同模式，不能静默把截断文件当作完整录制。
- 大文件用 `StreamingRlogReader` / `StreamingReplayRhythm<Y, D>`，避免默认 eager 读取；按需开启 `recording-lz4`、`recording-zstd`。CPU 图像/视频类型位于 `roplat::media`，FFmpeg 适配由 `media-ffmpeg` 显式启用。
- `CausalRecorder` 另支持应用显式捕获单个串行域的输入、旁路观察版本、状态、虚拟时间和停止/失败。它不自动恢复任意并发图、设备状态或外部副作用。

本页核对依据：发布包 `roplat 0.5.1` 的源码提交为 `cfee536c79c4760ae7579367e27a8dd8bd3221f4`；当前主线见固定提交的[消息契约](https://github.com/Robot-Exp-Platform/roplat/blob/78fd47ee0a90194feb88f706ca90d6d584331de8/roplat_native/src/lib.rs)、[Python 桥](https://github.com/Robot-Exp-Platform/roplat/blob/78fd47ee0a90194feb88f706ca90d6d584331de8/roplat_py/src/native_bridge.rs)、[录制会话](https://github.com/Robot-Exp-Platform/roplat/blob/78fd47ee0a90194feb88f706ca90d6d584331de8/roplat/src/rhythm/replay/session.rs)及[IPC 配置](https://github.com/Robot-Exp-Platform/roplat/blob/78fd47ee0a90194feb88f706ca90d6d584331de8/roplat/src/comm/ipc/ring_buffer.rs)。这些是源码核对，不是本技能对录放、媒体、跨语言或真实设备的运行验收。
