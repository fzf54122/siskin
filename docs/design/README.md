<!--
@Time    : 2026/11/5 16:42
@Author  : fzf
@FileName: README.md
@Software: Siskin
-->

# Siskin 界面设计稿

参考 Otty 的柔和侧栏、轻边界与整体感。这是待确认的设计，不是 Rust 应用截图，
也不代表以下交互已实现。示例会话与终端内容仅用于说明布局。

直接在浏览器打开 `index.html`。原型支持切换工作区、外观和字体页面，
以及查看深色与强调色；不会启动 Shell、读取实际设置或保存配置。
其他控件仅展示外观。

## 效果图

- `terminal-light.png`：浅色工作区。
- `settings-light.png`：外观设置。
- `typography-light.png`：字体与排版。
- `terminal-dark.png`：深色工作区。
- `terminal-clear.png`、`settings-clear.png`、`typography-clear.png`：第二版浅色，
  使用清透白和极浅青绿色，未获认可。
- `terminal-minimal.png`、`settings-minimal.png`、`typography-minimal.png`：第三版浅色，
  纯白、无彩色容器、细线分区、单列表单。
- `terminal-refined.png`、`settings-refined.png`、`typography-refined.png`：第三版细化，
  加强左栏分隔线、选中项描边和文字对比度；保留终端彩色输出。
- `terminal-native.png`、`settings-native.png`、`typography-native.png`：沿用当前宿主终端
  Deepin Terminal 的 Light 文字配色与 Noto Sans Mono 等宽字体；用户反馈颜色过硬。
- `terminal-soft.png`、`settings-soft.png`、`typography-soft.png`：保持布局与白底，
  正文改为墨灰，蓝紫与黄色降低饱和度，选中项减少字重。

`index.html#terminal`、`#settings`、`#typography`、`#dark` 对应上述视图。
第三版使用 `index.html?style=plain#terminal`，也可切换为 `#settings` 或 `#typography`。

## 视觉约定

- 第三版使用白色框架、侧栏与画布，不再通过大片底色区分区域。
- 侧栏以深色竖线、轻描边和字重标识选中项，取消嵌套面板、阴影和大圆角。
- 当前稿的正文使用墨灰，辅助文字和彩色示例均保持可读对比度，避免纯黑与高饱和蓝紫。
- 会话使用单行列表，不重复显示完整路径、运行标签和品牌块。
- 路径只在工作区工具栏展示完整上下文，长路径保留末级目录。
- 图标统一线宽，常用操作提供悬停提示和明确的键盘焦点。
- 设置采用分类导航与单列表单，控件宽度和左右边界统一；仅在字体页显示字形预览。
- 图中的窗口控制仅示意系统装饰，实际应用继续由桌面绘制。

## 配色参考与调整

只读确认宿主进程为 `deepin-terminal`，用户配置选择 `Light` 与 `Noto Sans Mono`。
颜色取自已安装的 `/usr/share/terminalwidget5/color-schemes/Light.colorscheme`，
未修改用户的终端配置。

- 默认文字：`#000000`。
- 蓝色（ANSI 4）：`#1818B2`。
- 品红（ANSI 5）：`#E11EE1`。
- 黄色（ANSI 3）：`#B26818`。
- 次要说明文字使用中性深灰，不把 ANSI 彩色用于界面标签。
- 保留已认可的纯白界面背景；没有照搬宿主的灰白底色、透明度或字号。

用户认为直接照搬的纯黑与彩色过硬，当前稿改为：

| 用途 | 颜色 |
| --- | --- |
| 正文 | `#3B3D42` |
| 辅助文字 | `#62666F` |
| 次要信息 | `#70757E` |
| 蓝色示例 | `#496C9B` |
| 紫色示例 | `#87618D` |
| 黄色示例 | `#8F703F` |

选中项字重为 500，以竖线和描边保留辨识度；这不意味着实际终端应剥除程序的 SGR 粗体。
对比度检查用于保证可读性，不能据此保证长时间观看不会疲劳，仍需用户实际体验确认。

## 待实现的交互约定

- 新建会话继承当前目录，切换后输入焦点回到终端。
- 关闭不是重启。存在运行中任务时给出确认，关闭最后一个会话按窗口关闭语义处理。
- 会话信息按需展开，不抢占终端焦点；关闭后恢复原输入位置。
- 设置保留后台会话，返回终端恢复先前选区、滚动位置与焦点。
- 字体使用可搜索选择器；字号显示实际值，不显示内部档位。
- 颜色使用真实输入框，提供选中、校验、确认和取消状态。
- 改动即时生效；保存中、已保存、保存失败是不同状态，失败不能伪装成成功。
- 命令面板、快捷键与按钮使用同一动作入口；搜索与设置优先处理自己的按键。
