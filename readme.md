基于 **Air780E + Rust + SQLite + Webhook** 的短信自动转发守护进程。

它可以监听 4G 模块收到的新短信，将短信读取出来并持久化到本地 SQLite，然后自动转发到指定 Webhook。适合保号短信、验证码接收、通知转发、低成本短信网关等场景。

## 架构图

![Architecture](assets/bkg.png)

## 工作流程

```mermaid
stateDiagram-v2
    [*] --> pending: 收到短信并入库
    pending --> sending: Forwarder claim
    sending --> sent: webhook 成功
    sending --> pending: webhook 失败但可重试
    sending --> failed: 超过最大重试次数
    sending --> pending: Reaper 恢复超时任务
    [*] --> decode_failed: PDU 解码失败但 raw 已落库
```

## 核心特性

- Air780E AT 指令通信
- PDU 模式短信读取
- 支持中文短信 UCS2 解码
- 支持长短信 multipart 组装
- SQLite 本地持久化队列
- Webhook 自动转发
- 失败重试与卡死任务恢复
- 内置 Web 控制台（React 19 + zustand，仪表盘 / 短信 / 调制解调器 / 系统事件多页签）
- 低 CPU 占用，适合树莓派长期运行

## 运行

```bash
GG_GUARD_MODEM_PORT=/dev/ttyACM0 \
GG_GUARD_WEBHOOK_URL=http://127.0.0.1:8082/sms \
GG_GUARD_DATABASE_URL=sqlite:///tmp/gg-guard/sms.db?mode=rwc \
RUST_LOG=info,sqlx=warn \
./gg-guard
```

## 构建

```bash
cargo build --release
```

生成文件：

```text
target/release/gg-guard
```

## Web 控制台

`gg-guard` 自带一个多页面 Web 控制台，用于实时查看短信队列、状态统计和调制解调器状态。后端用 axum 暴露只读 JSON 接口，前端用 React 19 + react-router + zustand 实现，构建产物会被内嵌服务。

### 构建 Web 资源

```bash
# 在项目根目录执行（需要 Node.js / pnpm）
pnpm --dir web install
pnpm --dir web build          # 产物输出到 web/dist
```

仅在修改前端代码后需要重新构建；`gg-guard` 启动时会读取 `GG_GUARD_WEB_DIR`（默认 `web/dist`）下的静态资源。

### 启动方式

二进制启动后默认监听 `0.0.0.0:8080`，浏览器访问 `http://<设备IP>:8080/` 即可：

```bash
GG_GUARD_MODEM_PORT=/dev/ttyACM0 \
GG_GUARD_WEBHOOK_URL=http://127.0.0.1:8082/sms \
GG_GUARD_DATABASE_URL=sqlite:///var/lib/gg-guard/sms.db?mode=rwc \
GG_GUARD_WEB_DIR=web/dist \
./gg-guard
```

### HTTP 接口

| 方法 | 路径                       | 说明                                          |
| ---- | -------------------------- | --------------------------------------------- |
| GET  | `/api/health`              | 后端存活检查                                  |
| GET  | `/api/stats`               | 按状态分组的短信计数                          |
| GET  | `/api/messages`            | 分页短信列表，支持 `status`、`q`、`page`、`size` |
| GET  | `/api/messages/{id}`       | 单条短信详情                                  |
| GET  | `/api/modem/status`        | 调制解调器当前状态（SIM / 信号 / 注册）       |
| GET  | `/api/modem/events`        | 最近的状态变化事件                            |

### 前端开发模式

修改前端时建议用 Vite 的热更新（端口 5173，会代理 `/api` 到 `127.0.0.1:8080`）：

```bash
# 终端 1：跑后端
GG_GUARD_API_ADDR=127.0.0.1:8080 ./target/debug/gg-guard

# 终端 2：跑前端 dev server
pnpm --dir web dev
# 浏览器打开 http://127.0.0.1:5173/
```

## 配置

常用环境变量：

| 变量                    | 说明              | 示例                                     |
| ----------------------- | ----------------- | ---------------------------------------- |
| `GG_GUARD_MODEM_PORT`   | AT 串口路径       | `/dev/ttyACM0`                           |
| `GG_GUARD_WEBHOOK_URL`  | 短信转发 Webhook  | `http://127.0.0.1:8082/sms`              |
| `GG_GUARD_DATABASE_URL` | SQLite 数据库地址 | `sqlite:///tmp/gg-guard/sms.db?mode=rwc` |
| `GG_GUARD_API_ADDR`     | HTTP/Web UI 监听地址（留空则禁用） | `0.0.0.0:8080`               |
| `GG_GUARD_CORS_ORIGINS` | 允许的 CORS 来源，逗号分隔（留空则允许全部） | `https://sms.example.com`   |
| `GG_GUARD_WEB_DIR`      | 前端构建产物目录  | `web/dist`                               |
| `RUST_LOG`              | 日志级别          | `info,sqlx=warn`                         |

## 组件说明

| 组件             | 作用                                      |
| ---------------- | ----------------------------------------- |
| `AtClient`       | 独占串口，发送 AT 指令，接收 URC          |
| `AtActor`        | 初始化模块，监听新短信事件，读取/删除短信 |
| `SmsIngestActor` | 解码 PDU，保存短信，组装长短信            |
| `ForwarderActor` | 从 SQLite 拉取待转发短信并调用 Webhook    |
| `ReaperActor`    | 恢复卡住的 sending 任务                   |
| `HealthActor`    | 定期检查 SIM、信号和注册状态              |

## 可靠性设计

短信处理采用：

```text
先入库 -> 再删除 SIM -> 再异步转发
```

这样即使 Webhook 暂时失败，短信也不会丢失，而是保存在 SQLite 中等待重试。

## 调试

开发调试可以打开项目 debug 日志：

```bash
RUST_LOG=info,sqlx=warn,gg_guard=debug ./gg-guard
```

生产环境建议使用：

```bash
RUST_LOG=info,sqlx=warn
```

避免 AT 原始日志过多。

## 注意事项

- Linux 下需要确保当前用户有串口权限。
- 如遇串口被占用，可关闭 `ModemManager`。
- `AT+CLIP=1` 如果返回 `ERROR`，通常不影响短信接收。
- 长短信只有所有分片到齐后才会转发。
