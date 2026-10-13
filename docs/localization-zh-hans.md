# LightCraft 简体中文界面

LightCraft 的界面支持英语、简体中文、繁体中文（台湾）和日语。语言设置保存在 `ui.json` 的 `language` 字段中。

- 通过「编辑 → 语言」或「设置 → 一般 → 语言」切换，选择会保留到下次启动。
- 已翻译的范围与 `zh-hant.json` 键集完全一致（1246 条），两份目录的键与键序一一对应。
- 正文字重与半粗体都使用 SIL OFL 的 **Noto Sans CJK SC**（简体中文）与 BIZ UDPGothic（日语）。
  MIXEL 默认构建内嵌两种未修改的常规字体，来自 craft-fonts 的固定版本，完整 OFL 许可和 SHA-256
  校验记录保存在 `mixel/vendor/craft-fonts/`。无需额外下载或设置环境变量。
  `CRAFT_FONTS_DIR=../craft-fonts` 仍可指定外部 [craft-fonts](https://github.com/storytold/craft-fonts)
  字体库，覆盖默认子集并添加文档字体。Noto CJK 使用 `AGENTS.md` 中明确允许的例外；不包含 Adobe Source 字体。
- 翻译只发生在显示层：命令 ID、照片文件名、用户输入的元数据都不做改动。
- 尚未翻译的技术性错误、补充说明和发行说明仍以英文显示。
- 网页版将 Noto Sans CJK SC 作为独立的 `lightcraft_zh_hans.otf` 文件随站点分发，页面启动时加载；
  这样可显示简体中文，也避免把约 16 MB 字体放进受单文件大小限制的 WASM。部署时需上传整个
  `target/web-site/` 目录。默认构建也会分发中文字体及其完整许可。
- 日期分组标题与拍摄时间按中文格式显示（如 `2026年9月20日 星期日`）。

## 翻译的维护

静态界面文字在 `crates/ui-egui/locales/zh-hans.json`，含运行时数值的文字在
`crates/ui-egui/locales/zh-hans-formats.json`。两者都以英文原文为键。

含数值的文字遵循以下约定：

- 每个译文都是普通的 Rust 格式字符串，构建时由 `format!` 对照调用点的参数检查（占位符写错会编译失败）。
- **按名字**：`{n}`、`{label}`、`{total}` 等与英文原文中的名字一致。
- **按位置**：`{0}`、`{1}` … 对应调用点无名参数的先后顺序（从 0 开始），用于调整语序，
  例如 `Added {n} photo{} to “{}”` 译为 `已将 {n} 张照片添加到“{1}”{0:.0}`。
- 保留英文原文的格式说明（`{:.1}`、`{d:+}`、`{:.0}%`）。
- `format!` 不允许未使用的参数：英文用来表示单复数的那个参数（`"s"` / `""`）在中文里写成
  `{:.0}`（精度为 0 的字符串不输出任何内容）。

`LIGHTCRAFT_LANGUAGE=zh-hans lightcraft-cli snapshot ...` 可以渲染中文界面用于检查。

翻译是否完整、两种目录的键与占位符是否一致、字形是否齐全，都由
`cargo test -p lightcraft-ui-egui i18n::tests` 验证，默认字体也参与字形检查。
