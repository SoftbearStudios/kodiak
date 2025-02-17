.PHONY: all build clean rustup

all: build

build:
	cargo build

clean:
	rm -rf bin target

rustup:
	rustup override set nightly-2024-04-20
	rustup target add x86_64-unknown-linux-musl
