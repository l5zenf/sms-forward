# Actor 互引用问题：主流方案调研

## 当前问题

`src/main.rs` 用 kill+respawn 绕过 AtActor / HealthActor / SmsIngestActor 互相需要彼此 `ActorRef` 的循环依赖。这是反模式：会导致 actor 各跑 on_start 两次、URC 轮询 loop 起两份、tick 计时器在死掉的 actor 上泄漏。

---

## 方案对比

### 方案 A：Kameo 内置 Registry（推荐 ✅）

Kameo 0.14 在 `actor_ref.rs:88-137` 已经直接提供：

```rust
// 注册方
actor_ref.register("at_actor")?;          // 写入全局 ACTOR_REGISTRY

// 使用方（任何 actor、任何时刻）
let AtActor_ref = ActorRef::<AtActor>::lookup(&"at_actor")?.unwrap();
```

**演员在 `on_start` 里注册自己 + lookup 同伴**，不用 kill+respawn。全局静态 `ACTOR_REGISTRY: Lazy<Arc<Mutex<ActorRegistry>>>`，线程安全。

Erlang/OTP 的 `pg` / `gproc` / `global` 就是这个模式。actix 的 `SystemRegistry` 也是同样思路。

**优点**：
- 零依赖注入参数；演员构造器只接业务依赖（modem/repo/forwarder）
- 异步延迟 lookup：actor 可在第一次需要时才 lookup，避免启动顺序问题
- 可观测：debug 时输出 registry 可看到所有 actor

**缺点**：
- lookup 失败需要兜底（最简单：日志 warn，放弃这条消息）
- 弱类型：lookup 返回具体类型前不知道是否注册过

### 方案 B：PreparedActor two-phase（Kameo 也支持）

在 `spawn.rs:148-277` 看到：

```rust
let prepared_at = PreparedActor::new();        // 拿到 ActorRef 但还没 spawn
let prepared_ingest = PreparedActor::new();
let prepared_health = PreparedActor::new();

let at_ref = prepared_at.actor_ref().clone();     // 这是「注册表前的临时引用」
let ingest_ref = prepared_ingest.actor_ref().clone();
let health_ref = prepared_health.actor_ref().clone();

// 把三个 ref 注入到演员里再真正 spawn
prepared_at.spawn(AtActor::new(modem).with_ingest(ingest_ref).with_health(health_ref));
prepared_ingest.spawn(SmsIngestActor::new(repo).with_at(at_ref));
prepared_health.spawn(HealthActor::new(repo).with_at(at_ref));
```

**优点**：
- 显式依赖，编译期类型安全
- 启动顺序天然无歧义（构件期统一注入）

**缺点**：
- 启动代码膨胀
- 新增 actor 必须改 main 三处（PreparedActor 声明 + 注入 + spawn）

### 方案 C：消息里回带 actor_ref（"tell self"}

actor A 给 B 发消息时把 `ctx.actor_ref()` 一起带上，B 在 handler 里拿到 A 的 ref 然后存起来用。

Kameo 的 `message::Context` 提供 `actor_ref()`（message.rs:90）：

```rust
impl Message<RawSmsPduReceived> for SmsIngestActor {
    async fn handle(&mut self, msg: RawSmsPduReceived, ctx: Context<'_, Self, Self::Reply>) {
        if self.at_actor_ref.is_none() {
            // 把 sender 通过消息字段也可，或者依赖 ctx 之外的输入
        }
        // ...
    }
}
```

**适合场景**：纯请求/响应，比如 ForwarderActor → Repo → reply。**不适合** AtActor 与 SmsIngestActor 这种长期回拨关系 —— 每次 handle 都要检查 ref 是否已填充，且方向反着。

### 方案 D：服务定位器（不推荐）

把所有 actor_ref 塞进一个 `Arc<RwLock<HashMap<...>>>` 自己管 —— 重新发明方案 A 的轮子。

---

## 对比其他 Rust actor 框架

| 框架     | 主流做法                          | 文件位置 |
| -------- | --------------------------------- | -------- |
| **Kameo** | 内置 `ActorRef::register/lookup` | `actor_ref.rs:88-137`，全局 `ACTOR_REGISTRY` |
| **actix** | `SystemRegistry` + `ArbiterRegistry`，actor 类型做 key | actix-actors src registry |
| **ractor** | `ractor::registry::register_actor` + `where_is` | ractor crate registry module |
| **xtra** | 没有内置 registry，主流做法是传入 `Address` 注入或用 `tokio::sync::mpsc` 自管（社区要么走 main 注入要么用 once_cell 自己存） |
| **Axiom** | 类似 actix 的 `ActorSystem` 显式注册表 |
| Erlang/OTP | `pg` / `global` / `gproc` 进程组注册 | Erlang 标准库 |

**主流共识**：所有成熟 actor 框架都内置 registry；自己用 `PreparedActor` 加注入参数是辅助手段但不是常态。

---

## 推荐实施方案（方案 A）

### 改造点

1. **SmsIngestActor**：去掉 `with_at_actor` 构造参数，在 `on_start` 里 `ctx.actor_ref().register("sms_ingest")?;`，handle 里按需 `ActorRef::<AtActor>::lookup(&"at_actor")?`。

2. **AtActor**：同理，on_start 注册 `"at_actor"`，URC poll loop 里 lookup `"sms_ingest"`。

3. **HealthActor**：on_start 注册 `"health"`，HealthTick handler 里 lookup `"at_actor"` 发 QueryStatus。

4. **main.rs**：直接 4 个独立 `spawn()`，**不再 kill+respawn**，启动顺序不重要（lookup 是延迟的）。

### 启动顺序问题

延迟 lookup 意味着 AtActor 的 URC poll loop 第一轮可能找不到 sms_ingest。两种处理：

- **简单**：3 秒间隔回退重试，拿到 `Some` 才发；多花点延迟无伤大雅。
- **稳妥**：`on_start` 内做一次同步 lookup 等待 —— 但 Kameo 的 lookup 不是 async（它是同步 `Mutex::lock`），可以在 URC loop 启动前先 `loop { if let Some(r) = lookup("sms_ingest")? { break } sleep(...) }` 做就绪等待。

### 注册名字约定

| Actor           | 名字常量            |
| --------------- | ------------------- |
| `AtActor`       | `"at_actor"`        |
| `SmsIngestActor`| `"sms_ingest"`      |
| `ForwarderActor`| `"forwarder"`       |
| `ReaperActor`   | `"reaper"`          |
| `HealthActor`   | `"health"`          |

集中放在 `application/actors/messages.rs` 作为 `pub const`。

### 何时落地

代码能跑就先不动；等下次改 actor 拓扑时一刀切。重构预估：约 60 行变更，5 个文件。

---

## 结论

**用方案 A（Kameo registry）**。这是 Kameo/actix/ractor/Erlang 一致推荐的做法，符合 plain.txt 第 §10 表格里 "Kameo：mailbox + … Actor 拆分"的语义，且不需要从其他框架切回来。

当前 kill+respawn 写法只作 MVP 临时过渡。**建议在实机联调之前的下一次代码整理里改成 registry 模式**，主要理由：

1. 当前写法可能导致 URC poll loop 重复启动 —— AtActor 第一次 spawn 就已经 on_start 跑起来启动了 modem.poll_loop，kill 之后 modem 实例化状态保留（modem 在 Arc 里，kill 不会 drop Arc）但 loop 任务被 abort —— **可能已经在读 SIM 卡**。
2. HealthActor 同理，第一份 spawn 就已经在 mailbox 上等待。
3. kill+重 spawn 后旧 actor 的弱引用还能 lookup 到，混乱。

需要我现在直接重构这部分吗？
