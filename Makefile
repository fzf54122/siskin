# @Time    : 2026/11/9 14:10
# @Author  : fzf
# @FileName: Makefile
# @Software: Siskin

# Siskin 开发入口。`make` 无参数等于 `make check`（与 CI 同一条门禁链）。
#
# 常用：
#   make             跑完整门禁：fmt → clippy → test → build → structure
#   make check       同上
#   make run         起一个带调试符号的开发构建
#   make release     打 release 二进制
#   make deb         打 Debian 包到 target/debian
#   make gate        走 neo-runner 的循环门禁，机器退出
#   make clean-dev   清掉 target 里的 IDE 与设计临时产物，不影响构建缓存

CARGO        ?= cargo
PYTHON       ?= python3
NEO_RUNNER   ?= neo-runner

# 与 .agents/loop.yaml 保持一致，改这里记得同步那边，否则本地与 CI 会分叉。
STRUCTURE_MAX_LINES   ?= 400
STRUCTURE_MAX_FN_LINE ?= 60

.DEFAULT_GOAL := check
.PHONY: help check fmt fmt-fix clippy test build release run deb gate clean clean-dev

## help: 列出所有可用目标
help:
	@echo "目标："
	@echo "  check        完整门禁：fmt → clippy → test → build → structure"
	@echo "  fmt          检查格式，失败时打印 diff"
	@echo "  fmt-fix      自动格式化"
	@echo "  clippy       clippy -D warnings"
	@echo "  test         运行全部测试"
	@echo "  build        debug 构建"
	@echo "  release      release 构建"
	@echo "  run          运行调试构建"
	@echo "  deb          打 Debian 包到 target/debian"
	@echo "  gate         neo-runner 循环门禁（与 .agents/loop.yaml 同步）"
	@echo "  clean        清空 target"
	@echo "  clean-dev    只清 IDE / 设计临时产物，保留构建缓存"
	@echo ""
	@echo "变量：STRUCTURE_MAX_LINES=$(STRUCTURE_MAX_LINES) STRUCTURE_MAX_FN_LINE=$(STRUCTURE_MAX_FN_LINE)"

## check: 完整门禁。顺序与 .agents/loop.yaml 相同，任一步失败即停。
check: fmt clippy test build structure
	@echo "✓ 全部通过"

fmt:
	@$(CARGO) fmt --all --check

fmt-fix:
	@$(CARGO) fmt --all

clippy:
	@$(CARGO) clippy --all-targets --locked -- -D warnings

test:
	@$(CARGO) test --all-targets --locked

build:
	@$(CARGO) build --locked

release:
	@$(CARGO) build --release --locked

run:
	@$(CARGO) run --locked

deb: release
	@$(PYTHON) scripts/build_deb.py --skip-build

structure:
	@$(PYTHON) scripts/check_structure.py \
		--max-lines $(STRUCTURE_MAX_LINES) \
		--max-fn-lines $(STRUCTURE_MAX_FN_LINE)

gate:
	@$(NEO_RUNNER) run -f .agents/loop.yaml --output json

## clean-dev: 删 IDE 与设计过程中落在 target/ 的产物，不碰构建缓存。
## target/ 整个被 .gitignore 忽略，这里只清确认过的临时目录。
clean-dev:
	rm -rf target/flycheck0 target/design-qa
	rm -f target/otty-*.png target/otty-design-reference*.png target/layout-preview.log

clean:
	@$(CARGO) clean
