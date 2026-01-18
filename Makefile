BIN := bin/juno-keys

.PHONY: build
build:
	cargo build --release
	mkdir -p bin
	cp target/release/juno-keys $(BIN)

.PHONY: test
test:
	cargo test

.PHONY: fmt
fmt:
	cargo fmt

.PHONY: lint
lint:
	cargo clippy -- -D warnings
