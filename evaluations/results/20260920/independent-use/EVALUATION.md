# roplat-development 独立使用验收

日期：2026-09-20。同一位未参与此 skill 编写的评估者完成三个任务；该评估者参与核心语义审计并有项目背景，因此这是独立使用验证，**不是完全盲评，也不是三个不同模型的泛化证明**。

只阅读技能 SKILL.md、references 中的版本/语法/节律/生命周期/通信/验证指南，以及 evaluations/tasks.md；没有阅读 rubric.md，没有阅读或复用 assets 示例实现。按技能要求核实了核心公开 Rhythm 签名、CountRhythm 构造、Latest 和 SharedFrame 的公开方法。所有实现独立写到本目录，没有运行硬件、服务器、IPC、Python或Replay。

核心仍处于候选工作分支收束中；版本文件 verified_commit 当时为 null。下述结论是此时实际编译的本机核心及记录的源码指纹，最终候选冻结后应统一再次运行。技能不能凭空补齐核心尚未认可的功能。

## 验证命令与结果

```sh
cargo run --offline
cargo fmt --all
cargo clippy --locked --offline --all-targets -- -D warnings
cargo run --locked --offline
```

初次 Rust 编译和断言即全部成功，未因错误改写实现。格式化后 Clippy 零警告，锁文件重跑全部成功。Rust 项目直接依赖 Tokio 1.48.0（与技能自带示例1.52.1不同），独立 Cargo.lock 已保留。应用层源码没有任何 `.process(...)` 调用；这些名称只作为 Node trait 方法实现出现。每个应用图均由 `#[roplat::system]` 组织。

最终 stdout：

```text
A: outputs 12714/12858; calls=24; transient create/init/shutdown/drop=24
B: business Err=41; promoted Err skips consumer; child outcome=99; lifecycle exact
C: 8/5 calls; sampled [1, 2, 3, 4, 5]; final=8; old4KiB intact
PASS: independent skill tasks A, B, C
```

采样序列是本次观察，不是保证；断言只要求顺序不回退、调用次数、最终值及帧内容，不要求读取全部发布版本。

## A：分层采样

`src/task_a.rs` 使用自定义 SumRhythm：Input 和 Yield 为 u64，Feed 为 u64，Output 为本次所有 Feed 的和加各层偏置。outer=2轮、calibration=3轮、sampling=2轮，偏置分别10000/1000/100。所有 drive 都完整 await，先取回 N 再处理 Execution；没有手工节点调用链。

应用 System：

```rust
#[roplat::system]
async fn sample_layers(
    seed: &mut Seed, outer: &mut SumRhythm, calibration: &mut SumRhythm,
    sampling: &mut SumRhythm, worker: &mut Worker, trace: &Trace,
) -> RoplatResult<u64> {
    seed >> outer >> |outer_value| {
        outer_value >> calibration >> |calibrated_value| {
            calibrated_value >> sampling >> |sample_value| {
                let mut transient = Temporary::new(trace.clone());
                sample_value >> worker >> transient
            }
        }
    };
    Ok(outer.output)
}
```

两次 System 调用得到12714和12858，区别来自同一 Worker 继续累加调用次数；完整汇总含全部子 drive 和层级偏置，与某次单轮 Feed 不同。Worker24次、外/中/内 drive 累计2/4/12次。外部 Worker 的 init/shutdown由创建层各调用一次，System没有重复调用；最内层临时节点创建/启用/关闭/Drop各24次。

## B：业务 Result、显式失败、子域恢复

`src/task_b.rs` 的三个独立 System 核心图分别是：

```rust
bad >> handler;              // handler Input 就是业务 Result
bad.output? >> after;        // 错误提升，after.process 不执行
child >> { bad.output? };
child.outcome >> recover;    // recover Input 是完整 Execution<u64>
```

第一个返回41，第二个返回 quality 错误，第三个恢复成99。断言逐项比对每个本层对象的 init/process/shutdown事件及逆序关闭，证明即使 after未process仍在成功init后得到关闭。第三种只隔离子域业务执行失败，不伪造对创建层生命周期失败的捕获能力。

## C：独立兄弟控制域旁路交换

`src/task_c.rs` 的应用图：

```rust
#[roplat::system]
async fn independent_controls(
    mut publisher: Publish, mut sampler: Sample,
) -> RoplatResult<(Publish, Sample)> {
    let mut writing = CountRhythm::new(8);
    let mut reading = CountRhythm::new(5);
    writing >> |tick| { tick >> publisher; };
    reading >> |tick| { tick >> sampler; };
    Ok((publisher, sampler))
}
```

主流边只把各自节律的 tick 送入对应节点；两个兄弟域之间没有直接主流依赖边。Latest<(usize,SharedFrame)>承载外部最新状态和帧，读者主动采样，允许漏过中间版本。Publisher和Sampler的process均真实 `yield_now().await`，整个程序使用current_thread Tokio runtime；不以额外OS线程代表旁路场景。

Writer8轮、Reader5轮。单独的最终读者得到版本8和4096个字节值8；之前保存的4096字节0xaa快照在所有后续发布后仍完整，与新帧不是同一payload owner。共享句柄保持旧快照生存期，Latest没有实现全局冻结或逐发布确认。

## 对技能的反馈

- 宏优先、生命周期创建层、四种节律值、普通捕获值区别、业务Result/Execution区分都足以指导这三个任务；没有遇到必须绕过System的缺口。
- 不读资产也能按正式Rhythm契约实现有限汇总节律；“先归还N，再处理Execution”指导有效。
- 能区分独立兄弟域和无限子域阻塞父依赖；本次兄弟节点以本体传入传出，避免假定spawn可持有任意外部非static借用。
- 这些任务只验证被测Rust/旁路子集。Python透明Node桥、IPC、Replay、真实设备安全和任意拓扑不属于此结果。没有因为技能可用而把它们写成已认可能力。
- 建议最终固定core commit后重新运行本项目，并把复现代码、锁文件和摘要保留为回归输入；不要把临时原始日志/target纳入仓库。

## 最终工作树复验

其后再次使用相同任务源程序和锁文件，对完成局部修订的核心工作树执行 run 和 Clippy，全部通过。实际核心四 crate 源码摘要为 `b7b35e868303b07b0dafe30975545cfaa2ac4d963fb51071e81195b7e1ae0f9f`，从历史 `577d642` 出发但未提交；验证参数见 [independent-final.json](../independent-final.json)。作者决定延后版本成立和全部提交/PR，因此该结果不代表已发布版本。
