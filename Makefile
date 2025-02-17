.PHONY: fmt line_count rustup

fmt:
	cargo fmt

line_count:
	find . -name '*.rs' | xargs wc -l

rustup:
	rustup override set nightly-2024-04-20
