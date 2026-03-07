# Helix Editor - 项目上下文指南

## 项目概述

Helix 是一款受 Kakoune/Neovim 启发的现代终端文本编辑器，使用 Rust 编写。

- **版本**: 25.7.1
- **许可证**: MPL-2.0
- **仓库**: https://github.com/helix-editor/helix
- **文档**: https://docs.helix-editor.com/

### 核心特性

- Vim 风格的模态编辑
- 多光标选择
- 内置 Language Server Protocol (LSP) 支持
- 基于 tree-sitter 的智能增量语法高亮和代码编辑

## 项目结构

```
helix/
├── helix-stdx/      # 标准库扩展（类似 rust-analyzer 的 stdx）
├── helix-core/      # 核心编辑原语，函数式设计
├── helix-parsec/    # 解析器组合子库
├── helix-loader/    # 外部资源的构建、获取和加载
├── helix-lsp-types/ # LSP 类型定义
├── helix-lsp/       # LSP 客户端实现
├── helix-dap-types/ # DAP 类型定义
├── helix-dap/       # Debug Adapter Protocol 客户端
├── helix-event/     # 编辑器内部事件原语和钩子
├── helix-vcs/       # 版本控制系统集成 (git)
├── helix-view/      # UI 抽象层，命令式 Shell
├── helix-tui/       # TUI 原语 (fork 自 tui-rs)
├── helix-term/      # 终端前端 (主入口点)
├── xtask/           # 自定义构建任务
├── runtime/         # 运行时资源
│   ├── grammars/    # tree-sitter 语法文件
│   ├── queries/     # 语法高亮/缩进查询
│   └── themes/      # 主题文件
├── book/            # mdBook 文档源码
└── contrib/         # 打包和补全脚本
```

### Crate 依赖关系

```
helix-term (终端应用入口)
├── helix-view (UI 抽象)
│   ├── helix-core (核心编辑原语)
│   │   ├── helix-stdx
│   │   ├── helix-loader
│   │   └── helix-parsec
│   ├── helix-lsp
│   │   ├── helix-lsp-types
│   │   └── helix-core
│   ├── helix-dap
│   │   └── helix-dap-types
│   ├── helix-vcs
│   └── helix-event
└── helix-tui (TUI 渲染)
```

## 构建和运行

### 基本命令

```bash
# 开发模式运行 (编译更快)
cargo run

# 发布版本构建
cargo build --release

# 运行测试
cargo test --workspace

# 运行集成测试
cargo integration-test

# 代码检查
cargo clippy --workspace --all-targets -- -D warnings

# 格式化检查
cargo fmt --all --check

# 生成文档
cargo doc --open
```

### Tree-sitter 语法

```bash
# 获取语法文件
cargo run -- --grammar fetch

# 构建语法文件
cargo run -- --grammar build
```

### 文档生成

```bash
# 生成自动文档
cargo xtask docgen

# 验证查询文件
cargo xtask query-check

# 验证主题文件
cargo xtask theme-check

# 预览书籍 (需要安装 mdbook)
mdbook serve book
```

## 核心架构

### helix-core

核心编辑原语，设计灵感来自 CodeMirror 6。采用函数式设计：大多数操作不原地修改数据，而是返回新副本。

关键类型：
- `Rope` / `RopeSlice`: 文本缓冲区的核心数据结构 (来自 ropey 库)
- `Selection`: 多选区域，包含多个 `Range`
- `Range`: 选择范围，包含可移动的 `head` 和固定的 `anchor`
- `Transaction`: OT 风格的文档变更，支持撤销/重做
- `Syntax`: tree-sitter AST 接口

主要模块：
- `selection`: 选择区域管理
- `transaction`: 文档变更事务
- `syntax`: 语法高亮和 AST
- `movement`: 光标移动逻辑
- `textobject`: 文本对象 (函数、类等)
- `surround`: 包围操作 (括号、引号等)
- `indent`: 缩进计算
- `search`: 搜索功能

### helix-view

UI 抽象层，提供编辑器状态管理。

关键类型：
- `Editor`: 全局状态，管理所有文档、视图分割、配置、语言服务器注册
- `Document`: 文档表示，绑定 Rope、Selection、Syntax、History、LSP 等
- `View`: UI 中的一个分割窗口
- `Compositor`: 组件层管理器

### helix-term

终端前端实现。

关键文件：
- `main.rs`: 入口点，设置事件循环
- `commands.rs`: 所有编辑器命令实现
- `keymap.rs`: 键位映射
- `application.rs`: 应用主循环
- `ui/`: UI 组件 (文件选择器、弹窗等)
- `handlers/`: 事件处理器

### 命令实现模式

命令通常遵循以下模式：

```rust
pub fn command_name(cx: &mut Context) {
    let (view, doc) = current!(cx.editor);
    // 获取选择
    let selection = doc.selection(view.id);
    // 获取文本
    let text = doc.text().slice(..);
    // 创建事务
    let transaction = Transaction::change(...)
        .with_selection(selection.clone());
    // 应用事务
    doc.apply(&transaction, view.id);
}
```

## 开发约定

### 代码风格

- 遵循标准 Rust 格式化规范 (`cargo fmt`)
- Clippy 警告必须全部解决 (`-D warnings`)
- 新功能需要包含测试

### 测试规范

- 单元测试和文档测试: `cargo test --workspace`
- 集成测试: `cargo integration-test`
- 集成测试位于 `helix-term/tests/`
- 测试辅助函数在 `helix-term/tests/test/helpers.rs`

### 日志调试

```bash
# 设置日志级别
hx -v file.txt      # info 级别
hx -vv file.txt     # debug 级别
hx -vvv file.txt    # trace 级别

# 输出到指定文件
cargo run -- --log debug.log
```

代码中使用：
```rust
log::info!("message");
log::warn!("warning");
log::error!("error");
```

### 提交规范

- 提交信息应清晰描述改动内容和原因
- 参考现有提交风格

## 配置文件

- `Cargo.toml`: 工作空间配置
- `languages.toml`: 语言服务器和语法配置
- `theme.toml`: 默认主题
- `base16_theme.toml`: Base16 主题模板
- `rustfmt.toml`: Rust 格式化配置
- `flake.nix`: Nix 构建配置

## 调试技巧

### 检查编辑器健康状态

```bash
hx --health           # 检查所有配置
hx --health rust      # 检查 Rust 语言支持
hx --health all       # 检查所有语言
```

### 常见问题

- **语法高亮不工作**: 运行 `hx --grammar fetch && hx --grammar build`
- **LSP 不工作**: 检查 `hx --health <language>` 确认语言服务器配置
- **macOS 测试失败**: 可能需要 `ulimit -n 10240` 增加文件描述符限制

## 相关资源

- [架构文档](docs/architecture.md)
- [贡献指南](docs/CONTRIBUTING.md)
- [发布说明](docs/releases.md)
- [愿景文档](docs/vision.md)
- [在线文档](https://docs.helix-editor.com/)
- [FAQ](https://github.com/helix-editor/helix/wiki/FAQ)
- [故障排除](https://github.com/helix-editor/helix/wiki/Troubleshooting)
