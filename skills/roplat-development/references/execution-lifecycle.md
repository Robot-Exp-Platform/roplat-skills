# 生命周期、失败与协作停止

```rust
use roplat::{Completion, Execution, ExecutionContext, Lifecycle, Node, RoplatError};
// Execution<T> = Result<Completion<T>, RoplatError>
// Completion<T> = Completed(T) | Stopped
```

Node::process 返回用户自己的 Output；如果 Output=Result<数据,业务错误>，它仍是普通数据。`node.output?` 才将该错误提升到当前执行域。Rhythm::drive 在业务 Output 外始终还有 Execution，因此 `.output` 是默认传播的便捷端点，`.outcome` 则交出完整结果供父图处理。

- 失败会通知同域兄弟停止；等待在途调用完整返回，再汇合、归还对象。
- 父 stop 向子传播，child自己的stop不直接停止parent；`.outcome` 可处理child失败，但不能撤销父亲已经存在的stop。
- 同一child不要混合读取`.output`和`.outcome`。`.outcome`也不能捕获父创建层初始化child对象时的hook失败。
- 多个已报告兄弟错误保存在ParallelError，关闭错误和主错误保存在LifecycleError；不要用第一个错误覆盖其余信息。
- `ExecutionContext` stop粘性；新的独立drive要新建context，不能复用已停止的context期待自动恢复。

## 正式 Rhythm 契约

```rust
fn drive<'call, N, F, Fut>(
    &'call mut self, nodes: N, op_domain: F, input: Self::Input,
    resources: <Self::Resources as ResourceSet>::Refs<'call>,
    context: ExecutionContext,
) -> impl Future<Output = (Execution<Self::Output>, N)> + Send
where
    N: Send,
    F: FnMut(N, Self::Yield, ExecutionContext) -> Fut + Send,
    Fut: Future<Output = (Execution<Self::Feed>, N)> + Send;
```

每轮先检查停止，调用op_domain并完整await，**先取回N再处理Execution**。失败/停止请求给定context停止，然后带N返回。不能对拥有N的周期Future使用会丢弃失败分支的try_join或超时取消；设备等待可以设计为可取消，但需要确保它没有丢失在途节点所有权。完整实现参见技能例程FoldRhythm。

资源通过独立 Resources 参数借用；调用失败或停止不需要业务 Output 来交还它。N 仍是宏规划的不透明拥有状态，节律不能假设其布局。设备会话收尾仍由驱动负责。

## 资源使用者的实现形状

```rust
impl Node for MovePair {
    type Resources = (Arm, Arm);
    type Input = (Goal, Goal);
    type Output = Result<(), MotionError>;
    async fn process<'call>(
        &'call mut self,
        (left_goal, right_goal): Self::Input,
        (left, right): (&'call mut Arm, &'call mut Arm),
    ) -> Self::Output {
        left.move_to(left_goal)?;
        right.move_to(right_goal)?;
        Ok(())
    }
}
```

这是接口形状，Arm/Goal/MotionError 取用户真实类型；图中两条资源边分别接 `pair.resources.0/1`，业务目标接 `pair.input.0/1`。即使右臂运动失败，两臂本体仍由调用方持有；之前发出的左臂指令不会回滚。资源类型可以不同，也可以相同。不要通过 `Option<Arm>` 回收、复制或持有 `'static` 引用实现此接口。允许对传入的 `&mut Arm` 替换本体。

Rhythm 使用相同的资源声明和生命周期参数，但借用持续到整次 drive 完成；应在返回前完成会话退出与 N 恢复。内部永久拥有设备的节律保留内部字段，并声明 `Resources = ()`。

## 创建层负责hooks

手写Rust节点：

```rust
impl Lifecycle for Worker { type Error = RoplatError; }
impl Node for Worker {
    type Resources = ();
    type Input = i64;
    type Output = i64;
    async fn process(&mut self, input: i64, _: ()) -> i64 { input + 1 }
}
```

默认hooks为空，但trait显式实现。System本层创建并用于图的对象，按声明顺序init、逆序shutdown；传入对象只使用。on_init失败要清理自己部分建立的资源。不会每轮drive自动刷新状态。不要在节点process里模拟重复on_init/on_shutdown。

推荐可停止System返回 `Execution<T>`。兼容的 `RoplatResult<T>` 会把Stopped适配为RoplatError::Stopped。普通值返回边界遇失败/停止可能panic，通常不适合作为需要恢复的控制入口。panic、abort、Future drop和不合作的阻塞调用不在归还保证内；停止请求也不等于设备已经物理安全停止。
