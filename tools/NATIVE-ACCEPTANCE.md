# Native acceptance

Repo Radar needs the full OctoSense shell: `card-host` does not provide the
`wasm` service. These tools use the shell's Makepad instrument and a new private
profile. They do not control an existing user window, configure a provider, or
read personal app data. Python 3 and the existing `hub` binary are required.

Run the harness safety tests first:

```sh
python3 -m unittest discover -s tests -p 'test_native_harness.py' -v
```

For local development, point `HUB` at the built Hub CLI and `HOST` at the exact
OctoSense executable you intend to test. Use new directories outside this repo:

```sh
python3 tools/native_dev_fixture.py --bundle bundle --hub "$HUB" \
  --out /private/tmp/repo-radar-fixture-001
shasum -a 256 "$HOST"
python3 tools/native_acceptance.py --host "$HOST" --host-kind development \
  --expected-host-sha256 <the-sha256-above> --hub "$HUB" \
  --dev-fixture /private/tmp/repo-radar-fixture-001/fixture.json \
  --scenario tools/native-watchlist.json --out /private/tmp/repo-radar-native-001
```

The development helper copies unsigned source, runs the normal bundle gate, and
signs only the private copy with ephemeral fixture keys, then deletes those keys.
The gate still requires a genuine native screenshot and complete listing. This
explicit test fixture is **not GitHub publishing or public-catalog acceptance**.
Do not submit its manifest, catalog, signatures or receipt as publication proof.
The app's publishing workflow uses GitHub attestations instead.

At the first custom-drawn App Hub agent prompt, the driver saves a native PNG
and prints an instruction. Inspect that run's PNG and provide the observed
**Don't allow** coordinates in the requested JSON file. The driver clicks that
point in its owned window and reads the resulting host consent record to verify
refusal. It never writes consent records. Do not copy coordinates from another
window or replace this step with a preapproved profile.
Use the logical window dimensions in the adjacent `.geometry.json`; a Retina
PNG may have twice as many pixels as logical points.

The scenario installs through App Hub's normal UI, searches real public GitHub,
pins two repositories, selects and reorders them, opens releases, refreshes,
publishes to Glance, restarts, and unpins both. The normal F9 binding closes
the automatically opened Glance panel before further app input. Assertions combine actual widget text with read-only
inspection of the Rust component's persisted watchlist. Sidebar selectors are
scoped so a repeated project title cannot accidentally pass as a list row.
It requires a `D3BarChart` of at least 320×160 logical pixels entirely inside
the activity viewport on the first populated screen without app scrolling, backed by fetched PR data; use a host with
native D3 registration. RC4 does not provide that chart registration, and the
first retained RC4 run also exposed its component HTTPS provider panic.
Quick successive searches and selections observe stale-result behavior; they do
not force a deterministic out-of-order network response. The title check saves
every sample, rejects any other project's title immediately, and allows only
brief redraw gaps (at most 0.5 seconds) before requiring the title again.
Component unit tests
cover controlled failure and rate-limit states separately.

Native input uses `wait=0`: a hidden host can apply a click, key, text or scroll
event and then fail its optional synchronous frame presentation. The harness
dispatches each input once and checks its effects through separate state and
pixel observations. An input HTTP error fails the step and is recorded with
`input_replayed: false`; it never causes the input to be sent again.
Only read-only `/snap`, `/s` and `/g` observations retry HTTP 404, for at most
eight attempts with 250 ms between attempts (each request retains its 25-second
timeout). Every failed observation is recorded. Other HTTP errors, native error
responses and retry exhaustion remain failures; a retry is not an acceptance
assertion or proof that the input succeeded.

Every run has a new output directory. Its receipt binds the host, Hub, catalog,
scenario and source hashes; records steps, screenshots, failure and cleanup;
and preserves the driver/scenario bytes and first failing frame/log. Review screenshots separately before
assigning a UX score. `passed` means only that the recorded scenario passed.
It does not prove physical input, model answers, external-browser navigation,
Android, chart support in an older host, or public install/update acceptance.

For the 0.1.1 UX follow-up, use the same source and an owned copy of the completed
profile. Add six public projects through search/pin; inspect the first and last
rows and scroll the watchlist. Open Chat, decline its optional agent, and check
that one explanation and an inactive **Chat off** action appear while the draft
remains editable. Keep distinct drafts for two projects, switch Activity /
Releases / Chat, refresh while typing, then append text without clicking the
editor again. Verify the exact draft and focus, plus the activity scroll position
after switching tabs. Repeat at 990 and 810 logical pixels of app width. Inspect
the native chart, first PR, release Markdown and full composer in the PNGs;
widget geometry alone does not establish readability. Finally, open a published
Glance summary from a different selected project and check the destination and
its retained draft. No live-model result is implied by this denied-agent test.

For a protected catalog dry run, use `--public-catalog <genuine-mirror>
--catalog-scope github-dry-run` instead of `--dev-fixture`. After public
publication, use `--catalog-scope github-public`. Both verify `catalog-v2.json`
and supply no test trust anchor. A valid envelope can come from a dry run, so
the driver always leaves `public_publication_proof` false; the separate public
publication/readback receipt establishes that fact. Use `--host-kind released`
only for a verified executable downloaded from a public release. A verified
Actions package awaiting publication uses `--host-kind release-candidate`;
bind its receipt to the final public archive bytes, and repeat acceptance if
signing or packaging changes those bytes.
For the final ordinary-user path, use `--public-store` instead of either catalog
option, with `--expected-catalog-sha256` from the independent public readback.
This requires a released host and leaves every Hub origin/channel/trust override
unset. The app must be found and installed from the host's built-in public store;
afterward the driver verifies its cached catalog and compares its exact bytes
with the expected public envelope. A concurrent catalog change fails the run
and preserves the evidence. Do not use a mirrored dry run as this acceptance.
Attach the separate release-download and GitHub publication receipts. A single
install run does not establish update compatibility; preserve a first-version
profile and exercise an actual second release in a dedicated update scenario.

## 中文

本应用使用自己的 Rust Wasm 组件，因此验收必须运行完整 OctoSense，不能用缺少
`wasm` 服务的 `card-host` 代替。以上命令为每次运行创建独立数据目录，仅通过
Makepad 原生 instrument 操作自己启动的隐藏窗口，不读取个人资料，也不配置模型。

开发工具只复制未签名源码，在私有副本上执行正常商店检查，并生成随后删除的临时
测试密钥。真实截图、清单和权限检查仍然生效。这是明确隔离的开发夹具，**不代表
GitHub 发布、公开商店安装或更新已通过**；正式发布仍使用 GitHub attestation。

遇到 App Hub 自绘的 agent 授权提示时，请查看该次运行保存的原生截图，在工具指定
的 JSON 文件中填写“Don't allow”的实际坐标。工具点击后只读取授权记录确认拒绝，
不会伪造或修改记录。

场景验证搜索、固定项目、选择、排序、版本页、刷新、发布 Glance、重启持久化和取消固定。
Glance 发布后会自动打开面板；驱动通过正常 F9 快捷键关闭它，再继续应用输入。
每次运行要求真实 PR 数据对应的原生 `D3BarChart` 完整位于活动视口内，且至少为
320×160 逻辑像素，因此需要已注册 D3 的宿主。标题稳定性检查保留每次采样，
出现其他项目标题立即失败；只容许不超过 0.5 秒的重绘缺失，之后必须重新看到正确标题。
RC4 没有该注册，首次保留的 RC4 测试还暴露了组件 HTTPS 提供程序 panic。
原生输入使用 `wait=0`：隐藏宿主可能已经执行点击、按键、文字或滚动，却在可选的
同步呈现阶段失败。驱动每次只发送一次输入，再独立检查状态及像素。输入的 HTTP
错误使该步骤失败并记录 `input_replayed: false`，绝不自动重发输入。只有只读的
`/snap`、`/s`、`/g` 遇到 HTTP 404 时才重试，最多八次，间隔 250 毫秒；每次请求
仍有 25 秒超时。每次失败均保留记录，其他 HTTP 错误、原生错误响应及重试耗尽
仍然失败。观察重试不能代替行为断言，也不能证明输入已成功。
失败会保留驱动和场景原始字节、日志、截图和 receipt，不覆盖失败记录。自动通过不等于视觉 UX
验收；仍需审视实际截图，也不代表手机、模型回答、外部浏览器或公开更新已验证。

0.1.1 的 UX 补充验收应使用同一源码和已完成测试资料的独立副本。通过搜索固定六个
公开项目，检查列表首尾和滚动；拒绝可选 agent 后，Chat 应只显示一条说明和不可用的
“Chat off”按钮，草稿仍可编辑。为两个项目输入不同草稿，切换标签和项目，边刷新边
输入，再不点击编辑器直接追加文字，验证草稿、焦点和活动页滚动位置。分别以 990 和
810 逻辑像素宽度审视真实截图中的图表、首条 PR、Markdown 版本说明及完整输入区。
最后从另一个项目打开 Glance 摘要，确认目标项目及草稿。拒绝授权的测试不代表模型回答已验证。

受保护的目录试运行使用 `--public-catalog <genuine-mirror>
--catalog-scope github-dry-run`；正式发布后改用 `--catalog-scope github-public`。
两者均验证真实 `catalog-v2.json`，不提供测试信任锚点。有效证明也可能来自试运行，
因此驱动始终保留 `public_publication_proof: false`；公开发布事实须由独立发布及下载
回读 receipt 证明。保留下载哈希和 GitHub 证明，独立验证安装及
第二版本更新；不能把私有开发夹具的签名或 receipt 当作正式发布证明。

最终普通用户流程使用 `--public-store` 和公开回读 receipt 中的
`--expected-catalog-sha256`，不传任何目录选项。驱动要求已验证下载的发布宿主，
不设置 Hub 来源、频道或信任覆盖。安装后验证宿主实际缓存的目录，逐字节匹配
已发布目录；并发目录变化会使本次验收失败并保留证据，不能用镜像试运行代替。
尚未公开的 Actions 安装包使用 `--host-kind release-candidate`，不能标为
`released`。发布后绑定最终公开包的字节；签名或打包改变字节时须重新验收。
