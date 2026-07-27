# 模型 / Provider 绑定不变量

## 1. 不变量（一句话）

**任何 LLM 调用所需的「模型名」与「已绑定 endpoint / 凭证 / 协议的客户端」必须来自同一次 `LlmResolver::resolve`，并以 `ResolvedCall` 成对流动；禁止把裸 model 字符串交给另一路冻结的 `Arc<dyn LlmProvider>`。**

```
                    LlmResolver::resolve(scene, Some(catalog_id))
                                      │
                                      ▼
                         ResolvedCall { sealed }
                    ┌─────────────────┴─────────────────┐
                    │ provider_impl  (客户端)             │
                    │ model          (wire 名)            │
                    │ catalog_id     (catalog 主键)       │
                    └─────────────────┬─────────────────┘
                                      │
                                      ▼
                         AgentLoop::new(binding, ...)
```

## 2. 为什么必须这样

`models.toml` 里每个条目至少有三套彼此独立的字段：

| 字段 | 例子 | 用途 |
|------|------|------|
| `id` | `fcodex/gpt-5.6-sol` | catalog 主键、会话 override、thinking 档位键 |
| `model_name` | `gpt-5.6-sol` | 发给 API 的 wire 名 |
| `provider` + `base_url` + `api_key_env` + `api` | fcodex / fcodex.top / openai-responses | 真正的出口 |

把「名字」和「出口」拆开传递，就会出现：deepseek endpoint 收到 `fcodex/gpt-5.6-sol` → HTTP 400。

## 3. 事故复盘（2026-07）

配置：

- `llm.default_model = deepseek-v4-flash`
- 会话 `modelOverride = fcodex/gpt-5.6-sol`

旧路径：

1. `ChatContext::from_config` 启动时 `resolve(Main, None)`，只留下 `provider_impl`（deepseek）注入 4 个子 Agent deps；
2. 主 Agent 每回合正确 `resolve(Main, Some(override))`；
3. 子 Agent 派发时 `model = session_model`（catalog id），`llm = 冻结 deepseek`；
4. `openai::effective_model` 因 catalog_model_id 不匹配，原样把 `fcodex/gpt-5.6-sol` 打到 deepseek → 400。

半成品教训：`run_loop` 曾写注释「子 Agent 派发时会拿这个字符串回 catalog 查 provider」，并记录过一次 `claude-opus-4-8` 串台 404；但「回 catalog 查」只做了存 catalog id 这一半，重新 resolve provider 没实现，同类根因再次炸开。

## 4. 现行机制

| 角色 | 做法 |
|------|------|
| 主 Agent | 每回合 `resolve_call` → `AgentLoop::new(main_call, ...)` |
| 子 Agent（plan/code/explorer/verifier） | 派发时 `resolve_subagent_runtime` → `AgentLoop::new(binding, ...)` |
| 插件 HostApi | `llm_for_request` 一律走 `LlmResolver` |
| 测试逃生口 | 仅 `ResolvedCall::from_parts_unchecked`（`#[doc(hidden)]`） |

`ResolvedCall` 带私有 `sealed: Sealed`，模块外无法结构体字面量构造。`AgentLoopConfig` 不再持有裸 `model` 字段。

## 5. 同行实现调研（证据）

至少查了 6 家同级目录实现：

| 实现 | 结论 | 证据 |
|------|------|------|
| Codex | 子 Agent spawn 强制复制 `turn.model` **与** `turn.provider` | `codex-rs/core/src/tools/handlers/multi_agents_common.rs` `build_agent_shared_config` ≈220–225；测试 `build_agent_spawn_config_uses_turn_context_values`（`multi_agents_tests.rs` ≈4459） |
| Continue | `ILLM` 聚合 model+provider；切模型重建实例 | `ModelService.getSubagentModels` / `createLlmApi` |
| Cline | `ProviderConfig{providerId,modelId}`；model 不在 provider 时 throw | `configured-provider-registry.ts` `createHandlerConfig` |
| OpenCode | 类型强制 `{providerID, modelID}` 二元组 | `Provider.getModel`；`provider.test.ts` invalid model/provider |
| cc-fork-01 | 进程级单 provider，子 Agent inherit 父 resolved model | `utils/model/agent.ts` `getAgentModel` |
| pi | registry 双键 `(provider, modelId)` | `model-registry.ts` `find` |

**共同做法**：下游消费聚合描述符，不接受「裸 model + 旧 client」。Codex 虽在 session client 层部分分离，但子 Agent 路径有显式配对复制 + 测试。

## 6. 什么条件下应当推翻本决策

仅当下列**全部**成立时才可重新讨论「允许 model 与 provider 分头传」：

1. 全仓只剩**一个** LLM endpoint / 凭证（不再有多 provider catalog）；
2. wire 名与 catalog id 永远相等（无 `prefix/model`）；
3. 用类型系统证明不可能出现「名字跟 A、出口跟 B」的组合，并有等价于 Codex `build_agent_spawn_config_uses_turn_context_values` 的回归测试。

否则：缓解（「启动时也解析一次 session override」「给 reviewer 单独配模型」）都是在接受一个本就不该存在的分离能力。

## 7. 相关测试

- `resolve_main_with_session_override_returns_provider_bound_to_that_model`
- `binding_invariant_test`（扫描 `from_parts_unchecked` / `ResolvedCall {`）
- `subagent_uses_provider_matching_session_model_not_startup_default`
- `all_four_subagents_resolve_main_with_session_model`
- `resolve_failure_aborts_with_model_unresolved_and_keeps_plan_file`
- `first_llm_fatal_uses_no_transcript_hint_and_llm_error_stop_reason`
- `h13_plan_reviewer_follows_session_catalog_id_across_providers`
